use super::geo_keys::GeoKeyDirectory;
use super::ifd::{IFDTag, IFDValue, ImageFileDirectory, TIFFReader};
use super::proj::{Georeference, Geotransform};
use crate::sources::Source;
use crate::Error;

/// Functionality specific to reading Cloud Optimized Geotiffs
#[derive(Debug)]
pub struct COG {
    // overviews[0] is the full resolution image
    pub overviews: Vec<Overview>,
    #[allow(dead_code)]
    mask_overviews: Vec<Overview>,
    pub geo_keys: GeoKeyDirectory,
    pub source: Source,
    pub georeference: Georeference,
}

#[derive(Debug)]
pub struct Overview {
    pub width: u64,
    pub height: u64,
    pub tile_width: u64,
    pub tile_height: u64,
    pub nbands: u64,
    pub ifd: ImageFileDirectory,
    pub is_full_resolution: bool,
}

#[derive(Debug)]
pub struct OverviewDataReader {
    pub width: u64,
    pub height: u64,
    pub nbands: u64,
    tile_width: u64,
    tile_height: u64,
    tile_offsets: Vec<u64>,
    tile_bytes_counts: Vec<u64>,
}

impl Overview {
    pub async fn from_ifd(
        ifd: ImageFileDirectory,
        source: &mut Source,
        is_mask: bool,
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

        // Check BitsPerSample is 8
        match ifd.get_tag_value(source, IFDTag::BitsPerSample).await? {
            IFDValue::Short(v) => {
                if is_mask {
                    if !v.iter().all(|item| *item == 1) {
                        return Err(Error::UnsupportedTagValue(
                            IFDTag::BitsPerSample,
                            format!("{:?}", v),
                        ));
                    }
                } else if !v.iter().all(|item| *item == 8) {
                    return Err(Error::UnsupportedTagValue(
                        IFDTag::BitsPerSample,
                        format!("{:?}", v),
                    ));
                }
            }
            value => return Err(Error::TagHasWrongType(IFDTag::BitsPerSample, value)),
        }

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
            .get_usize_tag_value(source, IFDTag::SamplesPerPixel)
            .await?;
        // TODO: Could/Should check ExtraSamples to know how to interpret those extra samples
        // (e.g. alpha)

        // TODO: Use u64 instead of usize here
        Ok(Overview {
            width: ifd.get_usize_tag_value(source, IFDTag::ImageWidth).await? as u64,
            height: ifd.get_usize_tag_value(source, IFDTag::ImageLength).await? as u64,
            nbands: nbands as u64,
            tile_width: ifd.get_usize_tag_value(source, IFDTag::TileWidth).await? as u64,
            tile_height: ifd.get_usize_tag_value(source, IFDTag::TileLength).await? as u64,
            ifd,
            is_full_resolution,
        })
    }

    pub async fn make_reader(&self, source: &mut Source) -> Result<OverviewDataReader, Error> {
        // Note that as per the COG spec, those two arrays are likely *not* stored compactly next
        // to the header, so this will cause additional reads to the source
        let tile_offsets = self
            .ifd
            .get_vec_usize_tag_value(source, IFDTag::TileOffsets)
            .await?
            .iter()
            // TODO: Read directly as u64
            .map(|v| *v as u64)
            .collect();
        let tile_bytes_counts = self
            .ifd
            .get_vec_usize_tag_value(source, IFDTag::TileByteCounts)
            .await?
            .iter()
            // TODO: Read directly as u64
            .map(|v| *v as u64)
            .collect();
        Ok(OverviewDataReader {
            width: self.width,
            height: self.height,
            nbands: self.nbands,
            tile_width: self.tile_width,
            tile_height: self.tile_height,
            tile_offsets,
            tile_bytes_counts,
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
        // Note that tiles can be larger than the image, so we need to ignore out of bounds pixels
        for ti in tile_rect.i_from..tile_rect.i_to {
            if ti < out_rect.i_from || ti >= out_rect.i_to {
                continue;
            }
            // TODO: Given we assert PlanarConfiguration, can use some memcpy below
            for tj in tile_rect.j_from..tile_rect.j_to {
                if tj < out_rect.j_from || tj >= out_rect.j_to {
                    continue;
                }
                for b in 0..self.nbands {
                    let out_offset = (ti - out_rect.i_from) * out_rect.width() * self.nbands
                        + (tj - out_rect.j_from) * self.nbands
                        + b;
                    let tile_offset = (ti - tile_rect.i_from) * self.tile_width * self.nbands
                        + (tj - tile_rect.j_from) * self.nbands
                        + b;
                    out_data[out_offset as usize] = tile_data[tile_offset as usize];
                }
            }
        }
    }

    pub async fn read_image_part(
        &self,
        source: &mut Source,
        rect: &ImageRect,
    ) -> Result<Vec<u8>, Error> {
        if rect.j_to > self.width {
            return Err(Error::OutOfBoundsRead(format!(
                "x_from + width out of bounds: {} > {}",
                rect.j_to, self.width
            )));
        }
        if rect.i_to > self.height {
            return Err(Error::OutOfBoundsRead(format!(
                "y_from + height out of bounds: {} > {}",
                rect.i_to, self.height
            )));
        }
        // TODO: May want the caller to pass the output vector instead of allocating
        let nbytes = rect.width() * rect.height() * self.nbands;
        let mut out_data = vec![0u8; nbytes as usize];
        let start_tile_j = rect.j_from / self.tile_width;
        let start_tile_i = rect.i_from / self.tile_height;
        let end_tile_j = (rect.j_to as f64 / self.tile_width as f64).ceil() as u64;
        let end_tile_i = (rect.i_to as f64 / self.tile_height as f64).ceil() as u64;

        let tiles_across = (self.width + self.tile_width - 1) / self.tile_width;

        for tile_i in start_tile_i..end_tile_i {
            for tile_j in start_tile_j..end_tile_j {
                // As per the spec, tiles are ordered left to right and top to bottom
                let tile_index = tile_i * tiles_across + tile_j;
                let offset = self.tile_offsets[tile_index as usize];
                // Read compressed buf
                // TODO: We assume PlanarConfiguration=1 here
                let mut tile_data = vec![0u8; self.tile_bytes_counts[tile_index as usize] as usize];
                source.read_exact(offset, &mut tile_data).await?;

                let tile_rect = ImageRect {
                    i_from: tile_i * self.tile_height,
                    j_from: tile_j * self.tile_width,
                    i_to: (tile_i + 1) * self.tile_height,
                    j_to: (tile_j + 1) * self.tile_width,
                };
                // "Decompress" into data
                self.paste_tile(&mut out_data, &tile_data, rect, &tile_rect);
            }
        }
        Ok(out_data)
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
            match ifd
                .get_tag_value(&mut source, IFDTag::PhotometricInterpretation)
                .await?
            {
                IFDValue::Short(v) => match v[..] {
                    // RGB
                    [2] => {
                        overviews.push(Overview::from_ifd(ifd, &mut source, false).await?);
                    }
                    // Mask
                    [4] => {
                        mask_overviews.push(Overview::from_ifd(ifd, &mut source, true).await?);
                    }
                    _ => {
                        return Err(Error::UnsupportedTagValue(
                            IFDTag::PhotometricInterpretation,
                            format!("{:?}", v),
                        ));
                    }
                },
                value => {
                    return Err(Error::TagHasWrongType(
                        IFDTag::PhotometricInterpretation,
                        value,
                    ))
                }
            }
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
                if overviews[i].nbands != overviews[0].nbands {
                    return Err(Error::NotACOG(format!(
                        "Overview {} has inconsistent nbands={}, expected {}",
                        i, overviews[i].nbands, overviews[0].nbands
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
        // As per the COG spec, the overview contains the projection/geokey data
        let geo_keys = GeoKeyDirectory::from_ifd(&overviews[0].ifd, &mut source).await?;

        let georeference = Georeference::decode(&overviews[0].ifd, &mut source, &geo_keys).await?;

        Ok(COG {
            overviews,
            mask_overviews,
            source,
            geo_keys,
            georeference,
        })
    }

    pub fn width(&self) -> u64 {
        self.overviews[0].width
    }

    pub fn height(&self) -> u64 {
        self.overviews[0].height
    }

    pub fn nbands(&self) -> u64 {
        self.overviews[0].nbands
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
}
