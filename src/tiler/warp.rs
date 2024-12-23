/// The GDAL documentation is a good source of inspiration for how to do warping
/// https://github.com/OSGeo/gdal/blob/b63f9ad1881853f000b054c7dd787090da1fb9dc/alg/gdalwarper.cpp#L1215
use proj::Transform;

use crate::tiff::georef::Georeference;
use crate::Error;

use super::{BoundingBox, TMSTileCoords, Vec2f};

/// This Warps from TMS (so 3857) to a raster in the given Crs/Georeference
pub struct Warper<'a> {
    transform: Transform,
    georef: &'a Georeference,
}

impl Warper<'_> {
    pub fn new(georef: &Georeference) -> Result<Warper, Error> {
        Ok(Warper {
            transform: Transform::new(3857, georef.crs.epsg_code())?,
            georef,
        })
    }

    /// Project a pixel at (px, py) in the tile to image coordinates
    /// px, py should be in [0, 256] since we use a 256 tile width
    pub fn project_tile_pixel(&self, tile: &TMSTileCoords, px: f64, py: f64) -> Vec2f {
        let (x_3857, y_3857) = tile.tile_pixel_to_3857_meters(px, py);
        self.project_3857_meters(x_3857, y_3857)
    }

    /// Project a point in 3857 meters coordinate to image coordinates
    pub fn project_3857_meters(&self, x_3857: f64, y_3857: f64) -> Vec2f {
        let (x_proj, y_proj) = self.transform.transform((x_3857, y_3857));
        // Reverse the geotransform, see https://gdal.org/tutorials/geotransforms_tut.html
        // x_proj = ul_x + overview_pixel_x * x_res;
        // y_proj = ul_y + overview_pixel_y * y_res;
        //
        // Here we reverse that to find overview_pixel_ from x/y_proj
        // => (x_proj - ul_x) / x_res = overview_pixel_x
        let overview_pixel_x =
            (x_proj - self.georef.geo_transform.ul_x) / self.georef.geo_transform.x_res;

        let overview_pixel_y =
            (y_proj - self.georef.geo_transform.ul_y) / self.georef.geo_transform.y_res;
        Vec2f {
            x: overview_pixel_x,
            y: overview_pixel_y,
        }
    }

    // For a given TMS tile, computes the bounding box on the source image specified by
    // `source_crs` and `source_georef`
    pub fn compute_image_pixel_bounding_box(&self, tile_coords: &TMSTileCoords) -> BoundingBox {
        let tile_bounds = tile_coords.tile_bounds_3857();
        let edges = tile_bounds.edges();
        // We use a similar algorithm as GDAL and project 21 points against each edge of the tile
        // onto the destination and compute the bbox from that
        // TODO: Use proj_trans_bounds here ?
        const N: usize = 21;
        let mut points: Vec<Vec2f> = vec![];
        for (c1, c2) in edges {
            let dir = c2 - c1;
            for n in 0..N {
                let p = c1 + dir * ((n as f64 + 1.0) / (N as f64));
                points.push(p);
            }
        }
        // project points to image
        let image_points: Vec<Vec2f> = points
            .iter()
            .map(|p| self.project_3857_meters(p.x, p.y))
            .collect();
        BoundingBox::from_points(&image_points)
    }
}

#[cfg(test)]
mod tests {
    use testutils::assert_float_eq;

    use crate::{
        epsg::spheroid_3857::{EARTH_EQUATOR_CIRCUMFERENCE, TOP_LEFT_METERS},
        epsg::{Crs, UnitOfMeasure},
        tiff::georef::Geotransform,
    };

    use super::*;

    #[test]
    fn test_compute_image_pixel_bounding_box_3857() {
        // Simulate an image covering the whole extent of 3857
        let image_size = 1000.0;
        let georef = Georeference {
            crs: Crs::PseudoMercator,
            unit: UnitOfMeasure::LinearMeter,
            geo_transform: Geotransform {
                ul_x: TOP_LEFT_METERS.0,
                ul_y: TOP_LEFT_METERS.1,
                x_res: EARTH_EQUATOR_CIRCUMFERENCE / image_size,
                y_res: EARTH_EQUATOR_CIRCUMFERENCE / image_size,
            },
        };
        let warper = Warper::new(&georef).unwrap();
        // Getting the (0, 0, 0) tile should just cover the whole image in one tile
        let bbox = warper.compute_image_pixel_bounding_box(&TMSTileCoords::from_zxy(0, 0, 0));
        assert_float_eq(bbox.xmin, 0.0, 1e-5);
        assert_float_eq(bbox.ymin, 0.0, 1e-5);
        assert_float_eq(bbox.xmax, image_size, 1e-5);
        assert_float_eq(bbox.ymax, image_size, 1e-5);
    }
}
