use super::geo_keys::GeoKeyDirectory;
use super::ifd::{IFDTag, IFDValue, ImageFileDirectory, TIFFReader};
use super::proj::Georeference;
use crate::sources::CachedSource;
use crate::Error;

/// Functionality specific to reading Cloud Optimized Geotiffs
#[derive(Debug)]
pub struct COG {
    // overviews[0] is the full resolution image
    pub overviews: Vec<Overview>,
    #[allow(dead_code)]
    mask_overviews: Vec<Overview>,
    pub geo_keys: GeoKeyDirectory,
    pub source: CachedSource,
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
        source: &mut CachedSource,
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

    pub async fn make_reader(
        &self,
        source: &mut CachedSource,
    ) -> Result<OverviewDataReader, Error> {
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

impl OverviewDataReader {
    // This pastes the given tile in the given output array, working on flattened version of the arrays
    fn paste_tile(&self, out: &mut [u8], tile: &[u8], img_i: u64, img_j: u64) {
        // Note that tiles can be larger than the image, so we need to ignore out of bounds pixels
        for ti in 0..self.tile_height {
            if img_i + ti >= self.height {
                break;
            }
            for tj in 0..self.tile_width {
                if img_j + tj >= self.width {
                    break;
                }
                for b in 0..self.nbands {
                    let out_offset =
                        (img_i + ti) * self.width * self.nbands + (img_j + tj) * self.nbands + b;
                    let tile_offset = ti * self.tile_width * self.nbands + tj * self.nbands + b;
                    out[out_offset as usize] = tile[tile_offset as usize];
                }
            }
        }
    }

    // TODO: Not sure caching make sense here
    pub async fn read_image(&self, source: &mut CachedSource) -> Result<Vec<u8>, Error> {
        let nbytes = self.width * self.height * self.nbands;
        let mut data = vec![0u8; nbytes as usize];
        let tiles_across = (self.width + self.tile_width - 1) / self.tile_width;
        let tiles_down = (self.height + self.tile_height - 1) / self.tile_height;
        for tile_i in 0..tiles_down {
            for tile_j in 0..tiles_across {
                // As per the spec, tiles are ordered left to right and top to bottom
                let tile_index = tile_i * tiles_across + tile_j;
                let offset = self.tile_offsets[tile_index as usize];
                println!("tile_i={}, tile_j={}, offset={}", tile_i, tile_j, offset);
                // Read compressed buf
                // TODO: We assume PlanarConfiguration=1 here
                let mut buf = vec![0u8; self.tile_bytes_counts[tile_index as usize] as usize];
                source.read_exact(offset, &mut buf).await?;
                // "Decompress" into data
                self.paste_tile(
                    &mut data,
                    &buf,
                    tile_i * self.tile_height,
                    tile_j * self.tile_width,
                );
            }
        }
        Ok(data)
    }
}

impl COG {
    pub async fn open(source_spec: &String) -> Result<COG, Error> {
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
}
