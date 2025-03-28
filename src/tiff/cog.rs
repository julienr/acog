use super::compression::Compression;
use super::data_types::data_type_from_ifd;
use super::data_types::InternalDataType;
use super::geo_keys::GeoKeyDirectory;
use super::georef::{Georeference, Geotransform};
use super::ifd::{IFDTag, IFDValue, ImageFileDirectory, TIFFReader};
use super::tags::PhotometricInterpretation;
use crate::bbox::BoundingBox;
use crate::image;
use crate::image::{DataType, ImageBuffer};
use crate::sources::Source;
use crate::Error;
use proj::Transform;

/// Functionality specific to reading Cloud Optimized Geotiffs
#[derive(Debug)]
pub struct COG {
    // overviews[0] is the full resolution image
    pub overviews: Vec<Overview>,
    pub mask_overviews: Vec<Overview>,
    pub geo_keys: GeoKeyDirectory,
    pub source: Source,
    pub georeference: Georeference,
    pub data_type: InternalDataType,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct BandsInterpretation {
    // An image consists of actual data band (that come first) and possibly an alpha band (that
    // comes last)
    // Typically:
    // - A RGBA image will have nbands=4, has_alpha=true
    // - A RGB image will have nbands=3, has_alpha=false
    // - A multispectral image will have nbands=10, has_alpha=false
    // - A mask band will have nbands=1, has_alpha=true
    pub nbands: usize,
    pub has_alpha: bool,
}

impl BandsInterpretation {
    pub fn new(
        nbands: usize,
        extra_samples: &[u64],
        photometric_interpretation: PhotometricInterpretation,
    ) -> Result<BandsInterpretation, Error> {
        match photometric_interpretation {
            PhotometricInterpretation::BlackIsZero => {
                // Multispectral images. Typically extra_samples will be all 0
                if !extra_samples.iter().all(|x| *x == 0) {
                    Err(Error::OtherError(format!(
                        "Expected all 0 extra_samples, got {:?}",
                        extra_samples
                    )))
                } else {
                    Ok(BandsInterpretation {
                        nbands,
                        has_alpha: false,
                    })
                }
            }
            PhotometricInterpretation::Rgb | PhotometricInterpretation::YCbCr => {
                match extra_samples {
                    [] => {
                        if nbands != 3 {
                            Err(Error::OtherError(format!(
                                "Got nbands != 3 ({:?}) for RGB or YCbCr color interpretation without extra samples",
                                nbands
                            )))
                        } else {
                            Ok(BandsInterpretation {
                                nbands: 3,
                                has_alpha: false,
                            })
                        }
                    }
                    // ExtraSamples=2 means "Unassociated alpha data"
                    // See section 18 of the TIFF spec for more details on associated ws unassociated alpha. As
                    // far as we're concerned, the only difference seems to be that associated alpha allows
                    // a half transparent pixels whereas unassociated is basically a mask either 0 or 255 but no
                    // in-between
                    [2] => {
                        if nbands != 4 {
                            Err(Error::OtherError(format!(
                                "Got nbands != 4 ({:?}) for RGB or YCbCr color interpretation with extra samples",
                                nbands
                            )))
                        } else {
                            Ok(BandsInterpretation {
                                nbands: 4,
                                has_alpha: true,
                            })
                        }
                    }
                    _ => Err(Error::OtherError(format!(
                        "Unable to interpret extra_samples for RGB image: {:?}",
                        extra_samples
                    ))),
                }
            }
            PhotometricInterpretation::Mask => {
                if nbands != 1 {
                    Err(Error::OtherError(format!(
                        "Got nbands != 1 ({:?}) for mask color interpretation",
                        nbands
                    )))
                } else if !extra_samples.is_empty() {
                    Err(Error::OtherError(format!(
                        "Got extra_samples for mask band: {:?}",
                        extra_samples
                    )))
                } else {
                    Ok(BandsInterpretation {
                        nbands: 1,
                        has_alpha: true,
                    })
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Overview {
    pub width: u64,
    pub height: u64,
    pub tile_width: u64,
    pub tile_height: u64,
    pub bands: BandsInterpretation,
    pub photometric_interpretation: PhotometricInterpretation,
    pub ifd: ImageFileDirectory,
    pub is_full_resolution: bool,
    pub compression: Compression,
    data_type: InternalDataType,
}

#[derive(Debug)]
pub struct OverviewDataReader {
    pub width: u64,
    pub height: u64,
    bands: BandsInterpretation,
    tile_width: u64,
    tile_height: u64,
    tile_offsets: Vec<u64>,
    tile_bytes_counts: Vec<u64>,
    compression: Compression,
    data_type: InternalDataType,
}

impl Overview {
    pub async fn from_ifd(
        ifd: ImageFileDirectory,
        source: &mut Source,
        photometric_interpretation: PhotometricInterpretation,
    ) -> Result<Overview, Error> {
        // Check planar configuration is contiguous pixels
        match ifd
            .get_tag_value(source, IFDTag::PlanarConfiguration)
            .await?
        {
            IFDValue::Short(v) => {
                if v[0] != 1 {
                    return Err(Error::UnsupportedTagValue(
                        IFDTag::PlanarConfiguration,
                        format!("{:?}", v),
                    ));
                }
            }
            value => return Err(Error::TagHasWrongType(IFDTag::PlanarConfiguration, value)),
        }
        // We only support Orientation = 1 which means the image has origin at top-left
        // (usual image processing axes)
        // Since its defaults to 1 if undefined, it needs to either be defined as 1 or not defined
        match ifd.get_tag_value(source, IFDTag::Orientation).await {
            Ok(IFDValue::Short(v)) => {
                if v[0] != 1 {
                    return Err(Error::UnsupportedTagValue(
                        IFDTag::Orientation,
                        format!("{:?}", v),
                    ));
                }
            }
            Err(Error::RequiredTagNotFound(_)) => {
                // Pass - defaults to 1 which is what we expect
            }
            Ok(other) => return Err(Error::TagHasWrongType(IFDTag::Orientation, other)),
            Err(e) => return Err(e),
        }
        let compression = Compression::from_ifd(source, &ifd).await?;
        // https://docs.ogc.org/is/21-026/21-026.html
        // 7.2.1. Requirement Reduced-Resolution Subfiles
        let is_full_resolution = match ifd.get_tag_value(source, IFDTag::NewSubfileType).await {
            Ok(v) => match v {
                IFDValue::Long(v) => v[0] & 0x1 == 0,
                value => return Err(Error::TagHasWrongType(IFDTag::PlanarConfiguration, value)),
            },
            Err(Error::RequiredTagNotFound(_)) => true,
            Err(e) => return Err(e),
        };

        // Check SamplesPerPixel
        let nbands = ifd
            .get_u64_tag_value(source, IFDTag::SamplesPerPixel)
            .await? as usize;
        // Check ExtraSamples
        let extra_samples = match ifd
            .get_vec_u64_tag_value(source, IFDTag::ExtraSamples)
            .await
        {
            Ok(v) => Ok(v),
            // It is optional - and if not present, means no extra samples
            Err(Error::RequiredTagNotFound(IFDTag::ExtraSamples)) => Ok(vec![]),
            Err(e) => Err(e),
        }?;
        println!(
            "nbands={:?}, extra_samples={:?}, compression={:?}",
            nbands, extra_samples, compression
        );
        let bands = BandsInterpretation::new(nbands, &extra_samples, photometric_interpretation)?;
        let data_type = data_type_from_ifd(&ifd, source).await?;

        Ok(Overview {
            width: ifd.get_u64_tag_value(source, IFDTag::ImageWidth).await?,
            height: ifd.get_u64_tag_value(source, IFDTag::ImageLength).await?,
            bands,
            photometric_interpretation,
            tile_width: ifd.get_u64_tag_value(source, IFDTag::TileWidth).await?,
            tile_height: ifd.get_u64_tag_value(source, IFDTag::TileLength).await?,
            ifd,
            is_full_resolution,
            compression,
            data_type,
        })
    }

    pub async fn make_reader(&self, source: &mut Source) -> Result<OverviewDataReader, Error> {
        // Note that as per the COG spec, those two arrays are likely *not* stored compactly next
        // to the header, so this will cause additional reads to the source
        let tile_offsets = self
            .ifd
            .get_vec_u64_tag_value(source, IFDTag::TileOffsets)
            .await?;
        let tile_bytes_counts = self
            .ifd
            .get_vec_u64_tag_value(source, IFDTag::TileByteCounts)
            .await?;
        Ok(OverviewDataReader {
            width: self.width,
            height: self.height,
            bands: self.bands,
            tile_width: self.tile_width,
            tile_height: self.tile_height,
            tile_offsets,
            tile_bytes_counts,
            compression: self.compression.clone(),
            data_type: self.data_type,
        })
    }
}

// TODO: Use x/y instead ?
// TODO: Use u32 here, u64 doesn't make sense for image dimensions
#[derive(Debug)]
pub struct ImageRect {
    pub i_from: u64,
    pub j_from: u64,
    pub i_to: u64,
    pub j_to: u64,
}

impl ImageRect {
    pub fn width(&self) -> u64 {
        self.j_to - self.j_from
    }
    pub fn height(&self) -> u64 {
        self.i_to - self.i_from
    }
}

impl OverviewDataReader {
    // Pastes the given tile at the right location in the output array. Both tile_rect and out_rect
    // define the area covered by out/tile in the whole image
    // Assumes both out_data and tile_data are packed as HwC (PlanarConfiguration=1)
    fn paste_tile(
        &self,
        out_data: &mut [u8],
        tile_data: &[u8],
        out_rect: &ImageRect,
        tile_rect: &ImageRect,
    ) {
        let size_bytes = self.data_type.unpacked_type().size_bytes();
        // Note that tiles can be larger than the image, so we need to ignore out of bounds pixels
        for ti in tile_rect.i_from..tile_rect.i_to {
            if ti < out_rect.i_from || ti >= out_rect.i_to {
                continue;
            }
            for tj in tile_rect.j_from..tile_rect.j_to {
                if tj < out_rect.j_from || tj >= out_rect.j_to {
                    continue;
                }
                let bytes_to_copy = self.bands.nbands * size_bytes;
                let out_offset = ((ti - out_rect.i_from)
                    * out_rect.width()
                    * self.bands.nbands as u64
                    * size_bytes as u64
                    + (tj - out_rect.j_from) * self.bands.nbands as u64 * size_bytes as u64)
                    as usize;
                let tile_offset = ((ti - tile_rect.i_from)
                    * self.tile_width
                    * self.bands.nbands as u64
                    * size_bytes as u64
                    + (tj - tile_rect.j_from) * self.bands.nbands as u64 * size_bytes as u64)
                    as usize;
                out_data[out_offset..(out_offset + bytes_to_copy)]
                    .copy_from_slice(&tile_data[tile_offset..(tile_offset + bytes_to_copy)]);
            }
        }
    }

    pub async fn read_image_part(
        &self,
        source: &mut Source,
        rect: &ImageRect,
    ) -> Result<ImageBuffer, Error> {
        if rect.j_to > self.width {
            return Err(Error::OutOfBoundsRead(format!(
                "rect.j_to out of bounds: {} > {}",
                rect.j_to, self.width
            )));
        }
        if rect.i_to > self.height {
            return Err(Error::OutOfBoundsRead(format!(
                "rect.i_to out of bounds: {} > {}",
                rect.i_to, self.height
            )));
        }
        // TODO: May want the caller to pass the output vector instead of allocating
        println!(
            "tile_width={}, tile_height={}, bands={:?}, data_type={:?}",
            self.tile_width, self.tile_height, self.bands, self.data_type
        );
        let mut out_data = {
            let nbytes = rect.width() as usize
                * rect.height() as usize
                * self.bands.nbands
                * self.data_type.unpacked_type().size_bytes();
            vec![0u8; nbytes]
        };
        let start_tile_j = rect.j_from / self.tile_width;
        let start_tile_i = rect.i_from / self.tile_height;
        let end_tile_j = (rect.j_to as f64 / self.tile_width as f64).ceil() as u64;
        let end_tile_i = (rect.i_to as f64 / self.tile_height as f64).ceil() as u64;

        let tiles_across = self.width.div_ceil(self.tile_width);

        // The below code assumes PlanarConfiguration=1 which is what GDAL does when creating COG, although
        // COGs with other planar configurations are possible in theory
        for tile_i in start_tile_i..end_tile_i {
            for tile_j in start_tile_j..end_tile_j {
                // As per the spec, tiles are ordered left to right and top to bottom
                let tile_index = tile_i * tiles_across + tile_j;
                let offset = self.tile_offsets[tile_index as usize];
                // Read compressed buf
                let mut tile_data = vec![0u8; self.tile_bytes_counts[tile_index as usize] as usize];
                // We use read_direct here to read the whole tile at once
                // TODO: Can this lead to too huge request depending on tile size ? Or does COG always
                // guarantee reasonable tile size ?
                source.read_exact_direct(offset, &mut tile_data).await?;

                // Decompress
                // TODO: Could reduce allocations by reusing the output vector across tiles (e.g. weezl support into_vec)
                tile_data = self.compression.decompress(
                    tile_data,
                    self.tile_width as usize,
                    self.tile_height as usize,
                )?;
                tile_data = self.data_type.unpack_bytes(&tile_data);

                let tile_rect = ImageRect {
                    i_from: tile_i * self.tile_height,
                    j_from: tile_j * self.tile_width,
                    i_to: (tile_i + 1) * self.tile_height,
                    j_to: (tile_j + 1) * self.tile_width,
                };
                let tile_data_expected_nbytes = tile_rect.width()
                    * tile_rect.height()
                    * self.bands.nbands as u64
                    * self.data_type.unpacked_type().size_bytes() as u64;
                if tile_data.len() as u64 != tile_data_expected_nbytes {
                    // If we fail here, two things could have happened:
                    // - The file has partial tiles that are less than tile_size * tile_size. That
                    //   valid as per the COG spec
                    // - Something is wrong with the decompression. Either the tile_data is compressed
                    //   but we didn't pick that up. Or the decompression didn't return enough data
                    //   (note that for now we don't support compression - so that's a note for when we do)
                    return Err(Error::InvalidData(format!(
                        "tile_data shorter than expected. {} instead of {}. Is there some compression issue ?",
                        tile_data.len(), tile_data_expected_nbytes
                    )));
                }
                self.paste_tile(&mut out_data, &tile_data, rect, &tile_rect);
            }
        }
        Ok(ImageBuffer {
            width: rect.width() as usize,
            height: rect.height() as usize,
            nbands: self.bands.nbands,
            data_type: self.data_type.unpacked_type(),
            has_alpha: self.bands.has_alpha,
            data: out_data,
        })
    }
}

impl COG {
    pub async fn open(source_spec: &str) -> Result<COG, Error> {
        let tiff_reader = TIFFReader::open_from_source_spec(source_spec).await?;
        // https://docs.ogc.org/is/21-026/21-026.html#_requirement_reduced_resolution_subfiles
        let mut overviews: Vec<Overview> = vec![];
        let mut mask_overviews: Vec<Overview> = vec![];
        let ifds = tiff_reader.ifds;
        let mut source = tiff_reader.source;
        for ifd in ifds {
            // Check photommetric interpretation to decide whether its the (RGB..) image or mask
            let photo_interp = PhotometricInterpretation::read_from_ifd(&mut source, &ifd).await?;
            match photo_interp {
                interp @ (PhotometricInterpretation::Rgb
                | PhotometricInterpretation::YCbCr
                | PhotometricInterpretation::BlackIsZero) => {
                    overviews.push(Overview::from_ifd(ifd, &mut source, interp).await?);
                }
                interp @ PhotometricInterpretation::Mask => {
                    mask_overviews.push(Overview::from_ifd(ifd, &mut source, interp).await?);
                }
            };
        }

        // COG requirement 3: first IFD must be full res image
        if !overviews[0].is_full_resolution {
            return Err(Error::NotACOG(
                "overview 0 is not full resolution".to_string(),
            ));
        }

        // COG requirement 3: IFD must be ordered by decreasing resolution
        // We also check that
        // - nbands are consistent
        // - this isn't a multi image COG - which we don't support
        {
            let mut prev_width = overviews[0].width;
            let mut prev_height = overviews[0].height;
            for i in 1..overviews.len() {
                if overviews[i].width >= prev_width {
                    return Err(Error::NotACOG(format!(
                        "Wrong overview ordering. Got overview i={} with width={} >= prev_width={}",
                        i, overviews[i].width, prev_width
                    )));
                }
                if overviews[i].height >= prev_height {
                    return Err(Error::NotACOG(format!(
                        "Wrong overview ordering. Got overview i={} with height={} >= prev_height={}",
                        i, overviews[i].width, prev_height
                    )));
                }
                if overviews[i].bands != overviews[0].bands {
                    return Err(Error::NotACOG(format!(
                        "Overview {} has inconsistent nbands={:?}, expected {:?}",
                        i, overviews[i].bands, overviews[0].bands
                    )));
                }
                if overviews[i].is_full_resolution {
                    return Err(Error::NotACOG(format!(
                        "Got a second full resolution overview (i={}). This library doesn't support multi image COGs",
                        i
                    )));
                }
                prev_width = overviews[i].width;
                prev_height = overviews[i].height;
            }
        }
        // As per the COG spec, the first overview contains the projection/geokey data
        let geo_keys = GeoKeyDirectory::from_ifd(&overviews[0].ifd, &mut source).await?;

        let georeference = Georeference::decode(&overviews[0].ifd, &mut source, &geo_keys).await?;

        let data_type = data_type_from_ifd(&overviews[0].ifd, &mut source).await?;

        Ok(COG {
            overviews,
            mask_overviews,
            source,
            geo_keys,
            georeference,
            data_type,
        })
    }

    pub fn width(&self) -> u64 {
        self.overviews[0].width
    }

    pub fn height(&self) -> u64 {
        self.overviews[0].height
    }

    pub fn bands_count(&self) -> usize {
        self.overviews[0].bands.nbands
    }

    pub fn compute_georeference_for_overview(&self, overview: &Overview) -> Georeference {
        let scale_factor = overview.width as f64 / self.width() as f64;
        Georeference {
            crs: self.georeference.crs,
            unit: self.georeference.unit,
            geo_transform: Geotransform {
                ul_x: self.georeference.geo_transform.ul_x,
                ul_y: self.georeference.geo_transform.ul_y,
                x_res: self.georeference.geo_transform.x_res / scale_factor,
                y_res: self.georeference.geo_transform.y_res / scale_factor,
            },
        }
    }

    // Obtain some statistics to be reported to the user
    pub fn get_stats(&self) -> String {
        self.source.get_stats()
    }

    pub fn lnglat_bounds(&self) -> Result<BoundingBox, Error> {
        let transform = Transform::new(self.georeference.crs.epsg_code(), 4326)?;

        let x1 = self.georeference.geo_transform.ul_x;
        let x2 = self.georeference.geo_transform.ul_x
            + self.georeference.geo_transform.x_res * self.width() as f64;
        let y1 = self.georeference.geo_transform.ul_y;
        let y2 = self.georeference.geo_transform.ul_y
            + self.georeference.geo_transform.y_res * self.height() as f64;
        let xmin = x1.min(x2);
        let xmax = x1.max(x2);
        let ymin = y1.min(y2);
        let ymax = y1.max(y2);
        transform
            .transform_bounds(&proj::MinMaxes {
                xmin,
                xmax,
                ymin,
                ymax,
            })
            .map(|v| BoundingBox {
                xmin: v.xmin,
                xmax: v.xmax,
                ymin: v.ymin,
                ymax: v.ymax,
            })
            .map_err(|e| e.into())
    }

    pub async fn make_reader(&mut self, overview_index: usize) -> Result<COGDataReader, Error> {
        let overview_reader = self.overviews[overview_index]
            .make_reader(&mut self.source)
            .await?;
        let mask_overview_reader = if self.mask_overviews.is_empty() {
            None
        } else {
            Some(
                self.mask_overviews[overview_index]
                    .make_reader(&mut self.source)
                    .await?,
            )
        };
        Ok(COGDataReader {
            bands: self.overviews[overview_index].bands,
            overview_reader,
            mask_overview_reader,
        })
    }

    /// Reads a part of a specific overview.
    /// Note that this will create a new overview reader, so this is inefficient if you want to
    /// read multiple parts: in that case, manage the overview reader yourself and call
    /// `read_image_part` on `OverviewDataReader`
    pub async fn read_image_part(
        &mut self,
        overview_index: usize,
        rect: &ImageRect,
    ) -> Result<ImageBuffer, Error> {
        if overview_index > self.overviews.len() {
            return Err(Error::OutOfBoundsRead(format!(
                "Invalid overview_index={}",
                overview_index
            )));
        }
        let overview = &self.overviews[overview_index];
        overview
            .make_reader(&mut self.source)
            .await?
            .read_image_part(&mut self.source, rect)
            .await
    }
}

// A helper class wraping an overview reader and a potential mask overview reader. This is
// to allow reading both the image and the mask at the same time
#[derive(Debug)]
pub struct COGDataReader {
    bands: BandsInterpretation,
    overview_reader: OverviewDataReader,
    // A reader for the mask data that corresponds to the same overview level as `overview_reader`
    // This is optional because typically:
    // - JPEG-encoded COG will have the mask in a separate overview (so mask_overview_reader will
    //   be defined)
    // - non-JPEG COGs can have the data just in RGBA (so mask_overview_reader will not be defined,
    //   all data coming from overview_reader)
    mask_overview_reader: Option<OverviewDataReader>,
}

impl COGDataReader {
    /// Return true if the result of `read_image_part` will have `has_alpha=true`
    /// TODO: Should we instead always read with alpha ?
    pub fn has_output_alpha(&self) -> bool {
        self.overview_reader.bands.has_alpha || self.mask_overview_reader.is_some()
    }

    pub fn output_bands(&self) -> usize {
        match self.mask_overview_reader {
            Some(_) => self.overview_reader.bands.nbands + 1,
            None => self.overview_reader.bands.nbands,
        }
    }

    pub async fn read_image_part(
        &self,
        source: &mut Source,
        rect: &ImageRect,
    ) -> Result<ImageBuffer, Error> {
        let image = self.overview_reader.read_image_part(source, &rect).await?;
        let maybe_mask = match &self.mask_overview_reader {
            Some(mask_reader) => Some(mask_reader.read_image_part(source, &rect).await?),
            None => None,
        };

        if let Some(mask) = maybe_mask {
            if mask.width != image.width || mask.height != image.height {
                return Err(Error::OtherError(format!(
                    "mask size ({}, {}) and image size ({}, {}) mismatch",
                    mask.width, mask.height, image.width, image.height
                )));
            }
            if mask.data_type != DataType::Uint8 {
                return Err(Error::OtherError(format!(
                    "Expected mask or uint8 data type ofor mask, got {:?}",
                    mask.data_type
                )));
            }

            if self.bands.has_alpha {
                return Err(Error::OtherError(format!(
                    "Image has both a mask and an alpha band. This is not supported"
                )));
            } else {
                if image.data_type != DataType::Uint8 {
                    // TODO: Implement that
                    return Err(Error::OtherError(format!(
                        "Non-uint8 masked images not supported yet {:?}",
                        image.data_type
                    )));
                }
                Ok(image::stack(&image, &mask)?)
            }
        } else {
            Ok(image)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ImageRect;

    #[tokio::test]
    async fn test_overview_reader_direct_reads() {
        // Test that reading from overview uses direct reads and not chunked ones
        let mut cog =
            crate::COG::open("example_data/example_1_cog_3857_nocompress_blocksize_256.tif")
                .await
                .unwrap();
        let overview = &cog.overviews[1];
        let ovr_reader = overview.make_reader(&mut cog.source).await.unwrap();
        assert_eq!(overview.width, 185);
        assert_eq!(overview.height, 138);
        ovr_reader
            .read_image_part(
                &mut cog.source,
                &ImageRect {
                    i_from: 0,
                    j_from: 0,
                    i_to: 100,
                    j_to: 100,
                },
            )
            .await
            .unwrap();
        let stats = cog.source.get_stats();
        // We should have one read for the IFD and then one direct read for the data.
        // If you change the direct data read into a chunked read, that would yield
        // 256 * 256 * 3 / 16384 = ~12 reads
        assert!(stats.contains("read_counts=2"));
    }
}
