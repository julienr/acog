use super::geo_keys::GeoKeyDirectory;
use super::ifd::{IFDTag, IFDValue, ImageFileDirectory, TIFFReader};
use crate::sources::CachedSource;
use crate::Error;
use super::proj::Georeference;

/// Functionality specific to reading Cloud Optimized Geotiffs
#[derive(Debug)]
pub struct COG {
    // overviews[0] is the full resolution image
    pub overviews: Vec<Overview>,
    #[allow(dead_code)]
    mask_overviews: Vec<Overview>,
    pub geo_keys: GeoKeyDirectory,
    pub source: CachedSource,
    pub georeference: Georeference
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
        // Check BitsPerSample
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
            // Check photometric interpretation indicates a RGB image
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
