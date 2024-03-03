use crate::epsg::Crs;
use crate::epsg::UnitOfMeasure;
use crate::sources::CachedSource;
use crate::tiff::cog::ImageRect;
use crate::Error;
use crate::COG;

/// TMS tile coordinates
/// Small notes on coordinate systems here.
/// - The 3857 coordinate system has x grow easts and y north
/// - The XYZ tile coordinates have x grow east left and y **south**
/// - The TMS tile coordinates have x grow east left and y north
///
/// Although XYZ is more popular to server tiles (that's what google maps/mapbox/osm use),
/// we use TMS internally for computations becaue the axis are going in the same direction so there
/// is less "flipping y" happening.
///
/// The XYZ to TMS conversion is fairly easy though: y_tms = 2 ** zoom - y_xyz - 1
///
/// Useful resources
/// - [maptiler XYZ and TMS viewer](https://www.maptiler.com/google-maps-coordinates-tile-bounds-projection/)
/// - [epsg.io 3857 interactive picker](https://epsg.io/map#srs=3857&x=-20037508.34&y=20048966.1&z=2&layer=streets)
/// - [OSM XYZ tiles numbering](https://wiki.openstreetmap.org/wiki/File:Tiled_web_map_numbering.png)
#[derive(Debug, Clone, Copy)]
pub struct TMSTileCoords {
    pub x: u64,
    pub y: u64,
    pub z: u32,
}

#[derive(Debug)]
struct BoundingBox {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

const TILE_SIZE: u64 = 256;

fn check_cog_is_3857(cog: &COG) -> Result<(), Error> {
    match cog.georeference.crs {
        Crs::PseudoMercator => (),
        Crs::Unknown(v) => {
            return Err(Error::UnsupportedProjection(format!(
                "Currently only support 3857, got {:?}",
                v
            )));
        }
    };

    match cog.georeference.unit {
        UnitOfMeasure::LinearMeter => (),
        UnitOfMeasure::Unknown(v) => {
            return Err(Error::UnsupportedProjection(format!(
                "Currently only support linear meters, got {:?}",
                v
            )));
        }
    };
    Ok(())
}

fn find_best_overview(cog: &COG, zoom: u32) -> usize {
    let tile_res_m = resolution(zoom);
    let cog_res_m = cog.georeference.geo_transform.x_res;
    println!("tile_res_m={}, cog_res_m={}", tile_res_m, cog_res_m);

    let mut selected_overview_index = 0;
    let mut selected_overview_res_m = cog_res_m;

    for (i, overview) in cog.overviews.iter().enumerate() {
        let overview_res_m = cog
            .compute_georeference_for_overview(overview)
            .geo_transform
            .pixel_resolution();
        if overview_res_m < tile_res_m && overview_res_m > selected_overview_res_m {
            selected_overview_index = i;
            selected_overview_res_m = overview_res_m;
        }
        println!("i={}, overview_res_m={}", i, overview_res_m);
    }
    println!("selected overview i={}", selected_overview_index);
    selected_overview_index
}

struct Point2D<T> {
    x: T,
    y: T,
}

pub async fn extract_tile(cog: &mut COG, tile_coords: TMSTileCoords) -> Result<Vec<u8>, Error> {
    check_cog_is_3857(cog)?;

    let overview_index = find_best_overview(cog, tile_coords.z);
    let overview = &cog.overviews[overview_index];
    let overview_georef = cog.compute_georeference_for_overview(overview);

    let tile_pixel_to_overview_pixel = |px: u64, py: u64| -> Point2D<f64> {
        let (x_proj, y_proj) = pixel_to_meters(
            tile_coords.x * TILE_SIZE + px,
            tile_coords.y * TILE_SIZE + py,
            tile_coords.z,
        );
        // Reverse the geotransform, see https://gdal.org/tutorials/geotransforms_tut.html
        // x_proj = ul_x + overview_pixel_x * x_res;
        // y_proj = ul_y + overview_pixel_y * y_res;
        //
        // Here we reverse that to find overview_pixel_ from x/y_proj
        // => (x_proj - ul_x) / x_res = overview_pixel_x
        let overview_pixel_x =
            (x_proj - overview_georef.geo_transform.ul_x) / overview_georef.geo_transform.x_res;

        let overview_pixel_y =
            (y_proj - overview_georef.geo_transform.ul_y) / overview_georef.geo_transform.y_res;
        Point2D {
            x: overview_pixel_x,
            y: overview_pixel_y,
        }
    };

    let nbands = overview.nbands;
    if nbands < 3 {
        return Err(Error::UnsupportedCOG(format!(
            "Require >= 3 bands, got {}",
            nbands
        )));
    }

    // As a first step, read the corresponding area from the overview
    let overview_area_ul = tile_pixel_to_overview_pixel(0, 0);
    let overview_area_br = tile_pixel_to_overview_pixel(TILE_SIZE, TILE_SIZE);
    // TODO: Clip ImageRect to overview bounds
    let overview_area_rect = ImageRect {
        j_from: std::cmp::max(0, overview_area_ul.x as u64),
        i_from: std::cmp::max(0, overview_area_br.y.ceil() as u64),
        j_to: std::cmp::min(overview.height, overview_area_br.x.ceil() as u64),
        i_to: std::cmp::min(overview.width, overview_area_ul.y as u64),
    };
    let overview_area_data = overview
        .make_reader(&mut cog.source)
        .await?
        .read_image_part(&mut cog.source, &overview_area_rect)
        .await?;

    // For each pixel in the output tile, interpolate its value from the overview_area_data we
    // just read
    // RGB image
    let mut tile_data: Vec<u8> = vec![0; (TILE_SIZE * TILE_SIZE * 3) as usize];
    for i in 0..TILE_SIZE {
        // TODO: Given we assert PlanarConfiguration, can use some memcpy below
        for j in 0..TILE_SIZE {
            // TODO: Naive nearest neighbor => replace by bilinear (or make this selectable)
            // Compute the 3857/projeced position of that pixel
            let overview_pixel = tile_pixel_to_overview_pixel(j, i);
            let overview_area_x = (overview_pixel.x as i64 - overview_area_rect.j_from as i64)
                .clamp(0, overview_area_rect.width() as i64 - 1);
            let overview_area_y = (overview_pixel.y as i64 - overview_area_rect.i_from as i64)
                .clamp(0, overview_area_rect.height() as i64 - 1);
            // Avoid out of bounds
            if overview_area_x < 0 || overview_area_x >= overview.width as i64 {
                continue;
            }
            if overview_area_y < 0 || overview_area_y >= overview.height as i64 {
                continue;
            }
            // We need to flip i here because i, j are in TMS coordinates with i/y growing north
            // but in raster space, y is growing south
            let i = TILE_SIZE - i - 1;
            for b in 0..3 {
                tile_data[(i * TILE_SIZE * 3 + j * 3 + b) as usize] = overview_area_data
                    [(overview_area_y as u64 * overview_area_rect.width() * nbands
                        + overview_area_x as u64 * nbands
                        + b) as usize];
            }
        }
    }

    Ok(tile_data)
}

// According to the spheroid used by 3857, see https://epsg.io/3857
const EARTH_RADIUS_METERS: f64 = 6378137.0;
const EARTH_EQUATOR_CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS;
// That's the "projected bounds" top left
const TOP_LEFT_METERS: (f64, f64) = (
    -EARTH_EQUATOR_CIRCUMFERENCE / 2.0,
    -EARTH_EQUATOR_CIRCUMFERENCE / 2.0,
);

/// Returns pixel size at a given zoom level of pyramid of EPSG:3857
fn resolution(zoom: u32) -> f64 {
    // Important, 256 is NOT TILE_SIZE, it is the number of pixels that are
    // covered at zoom level 0
    // See Leaflet's scale function:
    // https://github.com/Leaflet/Leaflet/blob/37d2fd15ad6518c254fae3e033177e96c48b5012/src/geo/crs/CRS.js#L62
    let initial_resolution = 2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS / 256.0;
    initial_resolution / (2.0_f64.powf(zoom as f64))
}

/// Convert pixel coordinates in given zoom level of pyramid to EPSG:3857
fn pixel_to_meters(x: u64, y: u64, zoom: u32) -> (f64, f64) {
    // Small notes on coordinate systems here.
    // The 3857 coordinate system has x grow left and y upwards
    // The XYZ tile coordinates have x grow left and y downwards
    //
    let res = resolution(zoom);
    let mx = x as f64 * res + TOP_LEFT_METERS.0;
    let my = y as f64 * res + TOP_LEFT_METERS.1;
    (mx, my)
}

impl TMSTileCoords {
    fn tile_bounds_3857(&self) -> BoundingBox {
        let (xmin, ymin) = pixel_to_meters(self.x * TILE_SIZE, self.y * TILE_SIZE, self.z);
        let (xmax, ymax) =
            pixel_to_meters((self.x + 1) * TILE_SIZE, (self.y + 1) * TILE_SIZE, self.z);
        BoundingBox {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }

    pub fn from_xyz(x: u64, y: u64, z: u32) -> TMSTileCoords {
        TMSTileCoords {
            x,
            y: 2u64.pow(z) - y - 1,
            z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolution, BoundingBox, TMSTileCoords};

    fn float_eq(v1: f64, v2: f64, epsilon: f64) -> bool {
        let diff = (v1 - v2).abs();
        diff <= epsilon
    }

    fn assert_float_eq(v1: f64, v2: f64, epsilon: f64) {
        if !float_eq(v1, v2, epsilon) {
            panic!(
                "{} != {} (difference={}, epsilon={})",
                v1,
                v2,
                (v1 - v2).abs(),
                epsilon
            );
        }
    }

    fn assert_bbox_equal(actual: &BoundingBox, expected: &BoundingBox, epsilon: f64) {
        if !(float_eq(actual.xmin, expected.xmin, epsilon)
            && float_eq(actual.ymin, expected.ymin, epsilon)
            && float_eq(actual.xmax, expected.xmax, epsilon)
            && float_eq(actual.ymax, expected.ymax, epsilon))
        {
            panic!("{:?} != {:?}", actual, expected);
        }
    }

    #[test]
    fn test_resolution() {
        // For reference: https://wiki.openstreetmap.org/wiki/Zoom_levels
        assert_float_eq(resolution(17), 1.194, 1e-2);
        assert_float_eq(resolution(20), 0.149, 1e-2);
    }

    #[test]
    fn test_tile_bounds_3857() {
        // Test tile bounds based on
        // https://www.maptiler.com/google-maps-coordinates-tile-bounds-projection/
        // https://epsg.io/map#srs=3857&x=-20037508.34&y=20048966.1&z=2&layer=streets
        // (click on a tile and look at 'spherical mercator (meters) bounds')
        assert_bbox_equal(
            &TMSTileCoords { x: 0, y: 0, z: 0 }.tile_bounds_3857(),
            &BoundingBox {
                xmin: -20037508.342789244,
                ymin: -20037508.342789244,
                xmax: 20037508.342789244,
                ymax: 20037508.342789244,
            },
            1e-5,
        );
        assert_bbox_equal(
            &TMSTileCoords { x: 0, y: 1, z: 1 }.tile_bounds_3857(),
            &BoundingBox {
                xmin: -20037508.342789244,
                ymin: 0.0,
                xmax: 0.0,
                ymax: 20037508.342789244,
            },
            1e-5,
        );
        assert_bbox_equal(
            &TMSTileCoords { x: 1, y: 1, z: 1 }.tile_bounds_3857(),
            &BoundingBox {
                xmin: 0.0,
                ymin: 0.0,
                xmax: 20037508.342789244,
                ymax: 20037508.342789244,
            },
            1e-5,
        );
        assert_bbox_equal(
            &TMSTileCoords { x: 17, y: 18, z: 5 }.tile_bounds_3857(),
            &BoundingBox {
                xmin: 1252344.0,
                ymin: 2504689.0,
                xmax: 2504689.0,
                ymax: 3757033.0,
            },
            1.0, // maptiler just gives integral coordinates
        );
    }
}