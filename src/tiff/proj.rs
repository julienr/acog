use super::geo_keys::{GeoKeyDirectory, KeyID};
use super::ifd::{IFDTag, ImageFileDirectory};
use crate::epsg::{Crs, UnitOfMeasure};
use crate::sources::Source;
use crate::Error;

/// A Geotransform, inspired by GDAL but enforcing north-up images
/// https://gdal.org/tutorials/geotransforms_tut.html
#[derive(Debug, Clone)]
pub struct Geotransform {
    // x coordinate of the upper left corner of the upper left pixel
    pub ul_x: f64,
    // y coordinate of the upper left corner of the upper left pixel
    pub ul_y: f64,
    // pixel dimensions
    pub x_res: f64,
    pub y_res: f64,
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-15
}

impl Geotransform {
    pub fn decode(tie_points: &Vec<f64>, pixel_scale: &[f64]) -> Result<Geotransform, Error> {
        if tie_points.len() != 6 {
            return Err(Error::UnsupportedProjection(format!("Currently only support rasters georeferenced with an affine geotransform. Expected tie_points of len 6, got {}", tie_points.len())));
        }
        if pixel_scale.len() != 3 {
            return Err(Error::UnsupportedProjection(format!("Currently only support rasters georeferenced with an affine geotransform. Expected pixel_scale of len 3, got {}", pixel_scale.len())));
        }
        // TODO: Here, need to look at raster_type to shift by 0.5 because the
        // geotransform ul_x is the coord of the uper left corner of upper left pixel
        if !close(tie_points[0], 0.0) || !close(tie_points[1], 0.0) || !close(tie_points[2], 0.0) {
            return Err(Error::UnsupportedProjection(format!(
                "Expected tie_points starting with [0, 0, 0]. Got {:?}",
                tie_points
            )));
        }
        if !close(tie_points[5], 0.0) || !close(pixel_scale[2], 0.0) {
            return Err(Error::UnsupportedProjection(format!(
                "Vertical crs not supported, expected 0, got tie_points[5]={}, pixel_scale[2]={}",
                tie_points[5], pixel_scale[2]
            )));
        }
        // TODO: Do we need to check axis mapping like GDAL (i.e. some CRS have 0 has x, some 0 as y ?)
        Ok(Geotransform {
            ul_x: tie_points[3],
            ul_y: tie_points[4],
            x_res: pixel_scale[0],
            // TODO: This should depend on the TIFF Orientation tag + CRS ?, but if not specified, it defaults to 1
            // which means that y grows downwards, which requires a - here because the geographic CRS have
            // y grow upwards (all of them ?)
            y_res: -pixel_scale[1],
        })
    }

    /// Return the average pixel resolution in the unit of the Georeference
    pub fn pixel_resolution(&self) -> f64 {
        (self.x_res.abs() + self.y_res.abs()) / 2.0
    }
}

#[derive(Debug, Clone)]
pub struct Georeference {
    pub crs: Crs,
    pub unit: UnitOfMeasure,
    pub geo_transform: Geotransform,
}

impl Georeference {
    pub async fn decode(
        ifd: &ImageFileDirectory,
        source: &mut Source,
        geo_keys: &GeoKeyDirectory,
    ) -> Result<Georeference, Error> {
        let model_type = geo_keys.get_short_key_value(KeyID::GTModelType)?;
        if model_type != 1 {
            // TODO: Could handle 2 (geodetic CRS)
            return Err(Error::UnsupportedProjection(format!(
                "Currently only support projected CRS (model_type=1), got {}",
                model_type
            )));
        }
        let raster_type = geo_keys.get_short_key_value(KeyID::GTRasterType)?;
        if raster_type != 1 {
            return Err(Error::UnsupportedProjection(format!(
                "Currently only support raster type 'RasterPixelIsArea' (1), got {}",
                raster_type
            )));
        }
        let crs = Crs::decode(geo_keys.get_short_key_value(KeyID::ProjectedCRS)?);
        let unit = UnitOfMeasure::decode(geo_keys.get_short_key_value(KeyID::ProjLinearUnits)?);
        // We are assuming that the geotransform is affine - which isn't necessarily the case.
        // See "B.6 GeoTIFF Tags for Coordinate Transformations" of the spec for more details
        let tie_points = ifd
            .get_vec_double_tag_value(source, IFDTag::ModelTiepointTag)
            .await?;
        let pixel_scale = ifd
            .get_vec_double_tag_value(source, IFDTag::ModelPixelScaleTag)
            .await?;
        let geo_transform = Geotransform::decode(&tie_points, &pixel_scale)?;
        Ok(Georeference {
            crs,
            unit,
            geo_transform,
        })
    }
}
