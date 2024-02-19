

use crate::epsg::{UnitOfMeasure, Crs};
use crate::sources::CachedSource;
use crate::Error;
use super::geo_keys::{GeoKeyDirectory, KeyID};
use super::ifd::{IFDTag, ImageFileDirectory};

#[derive(Debug)]
pub struct Georeference {
    crs: Crs,
    unit: UnitOfMeasure,
}

impl Georeference {
    pub async fn decode(ifd: &ImageFileDirectory, source: &mut CachedSource, geo_keys: &GeoKeyDirectory) -> Result<Georeference, Error> {
        let model_type = geo_keys.get_short_key_value(KeyID::GTModelType)?;
        if model_type != 1 {
            // TODO: Could handle 2 (geodetic CRS)
            return Err(Error::UnsupportedProjection(format!("Currently only support projected CRS (model_type=1), got {}", model_type)));
        }
        let raster_type = geo_keys.get_short_key_value(KeyID::GTRasterType)?;
        if raster_type != 1 {
            return Err(Error::UnsupportedProjection(format!("Currently only support raster type 'RasterPixelIsArea' (1), got {}", raster_type)));
        }
        let crs = Crs::decode(geo_keys.get_short_key_value(KeyID::ProjectedCRS)?);
        let unit = UnitOfMeasure::decode(geo_keys.get_short_key_value(KeyID::ProjLinearUnits)?);
        // TODO: Convert this in geotransform
        let tie_points = ifd.get_vec_double_tag_value(source, IFDTag::ModelTiepointTag).await?;
        let pixel_scale = ifd.get_vec_double_tag_value(source, IFDTag::ModelPixelScaleTag).await?;
        println!("tie_points: {:?}, pixel_scale: {:?}", tie_points, pixel_scale);
        Ok(Georeference{
            crs,
            unit
        })
    }
}