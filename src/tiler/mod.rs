use crate::bbox::BoundingBox;
use crate::epsg::spheroid_3857::{EARTH_RADIUS_METERS, TOP_LEFT_METERS};
use crate::image::ImageBuffer;
use crate::tiff::cog::ImageRect;
use crate::tiff::georef::Georeference;
use crate::Error;
use crate::COG;

use self::warp::Warper;
mod warp;
use crate::math::{vec2f, Vec2f};

/// TMS tile coordinates
/// Small notes on coordinate systems here.
/// - The 3857 coordinate system has x grow easts and y north
/// - The XYZ tile coordinates have x grow east left and y **south**
/// - The TMS tile coordinates have x grow east left and y north
///
/// Although XYZ is more popular to server tiles (that's what google maps/mapbox/osm use),
/// we use TMS internally for computations because the axis are going in the same direction so there
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

pub const TILE_SIZE: u64 = 256;

trait OverviewGeoreferenceCollection {
    fn georeference(&self) -> &Georeference;
    /// Returns the Georeferences for each overview in the COG
    fn georeferences_for_overview(&self) -> Vec<Georeference>;
}

impl OverviewGeoreferenceCollection for COG {
    fn georeference(&self) -> &Georeference {
        &self.georeference
    }

    fn georeferences_for_overview(&self) -> Vec<Georeference> {
        self.overviews
            .iter()
            .map(|o| self.compute_georeference_for_overview(o))
            .collect()
    }
}

fn find_best_overview(cog: &dyn OverviewGeoreferenceCollection, zoom: u32) -> usize {
    let tile_res_m = resolution(zoom);
    let cog_res_m = cog.georeference().geo_transform.x_res;
    println!("tile_res_m={}, cog_res_m={}", tile_res_m, cog_res_m);

    let mut selected_overview_index = 0;
    let mut selected_overview_res_m = cog_res_m;

    for (i, overview_georef) in cog.georeferences_for_overview().iter().enumerate() {
        let overview_res_m = overview_georef.pixel_resolution_in_meters();
        if overview_res_m < tile_res_m && overview_res_m > selected_overview_res_m {
            selected_overview_index = i;
            selected_overview_res_m = overview_res_m;
        }
        println!("i={}, overview_res_m={}", i, overview_res_m);
    }
    println!("selected overview i={}", selected_overview_index);
    selected_overview_index
}

pub struct TileData {
    pub img: ImageBuffer,
    #[allow(dead_code)]
    overview_index: usize,
}

pub async fn extract_tile(cog: &mut COG, tile_coords: TMSTileCoords) -> Result<TileData, Error> {
    let overview_index = find_best_overview(cog, tile_coords.z);
    let overview = &cog.overviews[overview_index];
    let overview_georef = cog.compute_georeference_for_overview(overview);

    // As a first step, read the corresponding area from the overview
    let (overview_area_ul, overview_area_br) = {
        let warper = Warper::new(&overview_georef)?;
        let image_bbox = warper.compute_image_pixel_bounding_box(&tile_coords);
        let bbox_ul = vec2f(image_bbox.xmin, image_bbox.ymax);
        let bbox_br = vec2f(image_bbox.xmax, image_bbox.ymin);
        (bbox_ul, bbox_br)
    };

    let overview_area_rect = ImageRect {
        j_from: std::cmp::max(0, overview_area_ul.x as u64),
        i_from: std::cmp::max(0, overview_area_br.y.ceil() as u64),
        j_to: std::cmp::min(overview.width, overview_area_br.x.ceil() as u64),
        i_to: std::cmp::min(overview.height, overview_area_ul.y as u64),
    };

    let nbands = overview.visual_bands_count() as u64;
    let dtype_size = cog.data_type.size_bytes();

    // Out of image tile => return transparent
    if overview_area_rect.j_to <= overview_area_rect.j_from
        || overview_area_rect.i_to <= overview_area_rect.i_from
    {
        return Ok(TileData {
            img: ImageBuffer {
                data: vec![0_u8; (TILE_SIZE * TILE_SIZE * nbands * dtype_size as u64) as usize],
                width: TILE_SIZE as usize,
                height: TILE_SIZE as usize,
                nbands: nbands as usize,
                data_type: cog.data_type,
            },
            overview_index,
        });
    }
    let overview_area_data = overview
        .make_reader(&mut cog.source)
        .await?
        .read_image_part(&mut cog.source, &overview_area_rect)
        .await?;

    // For each pixel in the output tile, interpolate its value from the overview_area_data we
    // just read
    let mut tile_data: Vec<u8> =
        vec![0; (TILE_SIZE * TILE_SIZE * nbands * dtype_size as u64) as usize];
    {
        let warper = Warper::new(&overview_georef)?;
        for i in 0..TILE_SIZE {
            // TODO: Given we assert PlanarConfiguration, can use some memcpy below
            for j in 0..TILE_SIZE {
                // TODO: Naive nearest neighbor => replace by bilinear (or make this selectable)
                // Compute the 3857/projeced position of that pixel
                let overview_pixel = warper.project_tile_pixel(&tile_coords, j as f64, i as f64);

                // If we are outside of the overview area rect, leave pixels black.
                // Note that we have a small 'margin' of one pixel to avoid black borders on the side
                // of some tiles
                let margin_px = 1.0;
                if overview_pixel.x < (overview_area_rect.j_from as f64 - margin_px)
                    || overview_pixel.x > (overview_area_rect.j_to as f64 + margin_px)
                {
                    continue;
                }
                if overview_pixel.y < (overview_area_rect.i_from as f64 - margin_px)
                    || overview_pixel.y > (overview_area_rect.i_to as f64 + margin_px)
                {
                    continue;
                }
                // We clamp again just out of caution to avoid out of bounds due to rounding errors or something
                let overview_area_x = (overview_pixel.x as i64 - overview_area_rect.j_from as i64)
                    .clamp(0, overview_area_rect.width() as i64 - 1);
                let overview_area_y = (overview_pixel.y as i64 - overview_area_rect.i_from as i64)
                    .clamp(0, overview_area_rect.height() as i64 - 1);

                // We need to flip i here because i, j are in TMS coordinates with i/y growing north
                // but in raster space, y is growing south
                let i = TILE_SIZE - i - 1;

                let tile_data_start_offset =
                    ((i * TILE_SIZE * nbands + j * nbands) * dtype_size as u64) as usize;
                let overview_data_start_offset =
                    ((overview_area_y as u64 * overview_area_rect.width() * nbands
                        + overview_area_x as u64 * nbands)
                        * dtype_size as u64) as usize;
                let nbytes = nbands as usize * dtype_size;
                tile_data[tile_data_start_offset..tile_data_start_offset + nbytes].copy_from_slice(
                    &overview_area_data.data
                        [overview_data_start_offset..overview_data_start_offset + nbytes],
                );
            }
        }
    }

    Ok(TileData {
        img: ImageBuffer {
            data: tile_data,
            width: TILE_SIZE as usize,
            height: TILE_SIZE as usize,
            nbands: nbands as usize,
            data_type: cog.data_type,
        },
        overview_index,
    })
}

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
pub fn pixel_to_meters(x: f64, y: f64, zoom: u32) -> (f64, f64) {
    // Small notes on coordinate systems here.
    // The 3857 coordinate system has x grow left and y upwards
    // The XYZ tile coordinates have x grow left and y downwards
    //
    let res = resolution(zoom);
    let mx = x * res + TOP_LEFT_METERS.0;
    let my = y * res + TOP_LEFT_METERS.1;
    (mx, my)
}

impl TMSTileCoords {
    // Create from XYZ coordinates
    pub fn from_zxy(z: u32, x: u64, y: u64) -> TMSTileCoords {
        TMSTileCoords {
            x,
            y: 2u64.pow(z) - y - 1,
            z,
        }
    }

    /// Convert from pixel coordinates within this tile to 3857 meters
    fn tile_pixel_to_3857_meters(&self, px: f64, py: f64) -> (f64, f64) {
        pixel_to_meters(
            (self.x * TILE_SIZE) as f64 + px,
            (self.y * TILE_SIZE) as f64 + py,
            self.z,
        )
    }

    fn tile_bounds_3857(&self) -> BoundingBox {
        let (xmin, ymin) = self.tile_pixel_to_3857_meters(0.0, 0.0);
        let (xmax, ymax) = self.tile_pixel_to_3857_meters(TILE_SIZE as f64, TILE_SIZE as f64);
        BoundingBox {
            xmin,
            ymin,
            xmax,
            ymax,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        epsg::{Crs, UnitOfMeasure},
        tiff::georef::{meters_to_lon_equator, Geotransform},
    };

    use super::*;
    use testutils::*;

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

    struct FakeCOG {
        georef: Georeference,
        overviews_georef: Vec<Georeference>,
    }

    impl OverviewGeoreferenceCollection for FakeCOG {
        fn georeference(&self) -> &Georeference {
            &self.georef
        }

        fn georeferences_for_overview(&self) -> Vec<Georeference> {
            self.overviews_georef.clone()
        }
    }

    fn make_meters_georeference(res_m: f64) -> Georeference {
        Georeference {
            crs: Crs::PseudoMercator,
            unit: UnitOfMeasure::LinearMeter,
            geo_transform: Geotransform {
                ul_x: 0.0,
                ul_y: 0.0,
                x_res: res_m,
                y_res: res_m,
            },
        }
    }

    #[test]
    fn test_find_best_overview_unit_meters() {
        let cog = FakeCOG {
            georef: make_meters_georeference(1.0),
            overviews_georef: vec![
                make_meters_georeference(2.0),
                make_meters_georeference(4.0),
                make_meters_georeference(8.0),
            ],
        };
        // Zoom level to size reference
        // https://wiki.openstreetmap.org/wiki/Zoom_levels
        assert_eq!(find_best_overview(&cog, 15), 1);
    }

    fn make_degrees_georeference(res_m_equator: f64) -> Georeference {
        let res_deg = meters_to_lon_equator(res_m_equator);
        Georeference {
            crs: Crs::Unknown(4326),
            unit: UnitOfMeasure::Degree,
            geo_transform: Geotransform {
                ul_x: 0.0,
                ul_y: 0.0,
                x_res: res_deg,
                y_res: res_deg,
            },
        }
    }

    #[test]
    fn test_find_best_overview_unit_degrees() {
        let cog = FakeCOG {
            georef: make_degrees_georeference(1.0),
            overviews_georef: vec![
                make_degrees_georeference(2.0),
                make_degrees_georeference(4.0),
                make_degrees_georeference(8.0),
            ],
        };
        // Zoom level to size reference
        // https://wiki.openstreetmap.org/wiki/Zoom_levels
        assert_eq!(find_best_overview(&cog, 15), 1);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_full_tile_3857() {
        // Tests extracting a tile that is fully covered by the image - which is already in 3857
        let mut cog = crate::COG::open("example_data/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_full_tile_3857_bigtiff() {
        // Tests extracting a tile that is fully covered by the image - which is already in 3857
        let mut cog = crate::COG::open("example_data/example_1_cog_3857_nocompress_bigtiff.tif")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_jpeg() {
        // JPEG compressed file
        let mut cog = crate::COG::open("example_data/example_1_cog_jpeg.tif")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_jpeg__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_deflate() {
        // DEFLATE compressed file
        let mut cog = crate::COG::open("example_data/example_1_cog_deflate.tif")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        // crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.nbands, expected.nbands);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_float32_multibands() {
        // Sentinel 2 file with 11 bands
        let mut cog = crate::COG::open("example_data/s2_corsica_1_multibands_deflate.tiff")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(15, 17166, 12168))
            .await
            .unwrap();
        let tile_rgb = tile_data.img.to_rgb(&[3, 1, 0], 0.0, 0.2).unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/s2_corsica_1_multibands_deflate.tiff 15 17166 12168 --bands 3,1,0 --vmin=0 --vmax=0.2
        //
        // crate::ppm::write_to_ppm("_test_img.ppm", &tile_rgb).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/s2_corsica_1_multibands_deflate__15_17166_12168.ppm",
        )
        .unwrap();
        assert_eq!(tile_rgb.width, expected.width);
        assert_eq!(tile_rgb.height, expected.height);
        assert_eq!(tile_rgb.nbands, expected.nbands);
        assert_eq!(tile_rgb.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_full_tile_ch1903() {
        // Tests extracting a tile that is fully covered by the image which is in CH1903+
        let mut cog = crate::COG::open("example_data/example_1_cog_nocompress.tif")
            .await
            .unwrap();
        // The image should be in CH1903+. Note that we check mostly to avoid wrongly using an
        // "already-in-3857" image
        // https://epsg.io/2056
        assert_eq!(cog.georeference.crs, Crs::Unknown(2056));
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_out_of_image() {
        // Tests extracting a tile that is fully out of the image
        let mut cog = crate::COG::open("example_data/example_1_cog_nocompress.tif")
            .await
            .unwrap();
        // This tile is fully outside of the image
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 0, 0))
            .await
            .unwrap();

        assert_eq!(tile_data.img.width, 256);
        assert_eq!(tile_data.img.height, 256);
        assert_eq!(tile_data.img.data, vec![0_u8; 256 * 256 * 3]);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_full_tile_4326() {
        // Tests extracting a tile that is fully covered by the image which is in 4326,
        // which means it also has UnitOfMeasure::Degree
        let mut cog = crate::COG::open("example_data/marina_1_cog_nocompress.tif")
            .await
            .unwrap();
        // The image should be in 4326. Note that we check mostly to avoid wrongly using an
        // "already-in-3857" image
        assert_eq!(cog.georeference.crs, Crs::Unknown(4326));
        assert_eq!(cog.georeference.unit, UnitOfMeasure::Degree);

        let tile_data =
            super::extract_tile(&mut cog, TMSTileCoords::from_zxy(21, 1726623, 1100526))
                .await
                .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/marina_1_cog_nocompress.tif 21 1726623 1100526
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();

        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/marina_1_cog_nocompress__21_1726623_1100526.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_full_tile_multiple_overviews() {
        // Test extracting a tile that requires looking at an overview > 0
        let mut cog =
            crate::COG::open("example_data/example_1_cog_3857_nocompress_blocksize_256.tif")
                .await
                .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(17, 68710, 45698))
            .await
            .unwrap();
        // This should have read from the second overview - not the full res image
        assert_eq!(tile_data.overview_index, 1);

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress_blocksize_256.tif 17 68710 45698
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress_blocksize_256__17_68710_45698.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_local_file_partial_tile() {
        // Tests extracting a tile that is only partially covered by the image
        let mut cog = crate::COG::open("example_data/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        // This specific tiles also covers the `margin_px` logic we have in `extract_tile``
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549689, 365591))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549689 365591
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress__20_549689_365591.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_minio_public() {
        // This is a file in a public bucket - not requiring working S3 authentication
        let mut cog = crate::COG::open("/vsis3/public/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();

        // To update this test, you can output the tile by uncommenting the following. You can
        // use the utils/extract_tile_rio_tiler.py to compare this tile to what riotiler
        // extracts and update the expected data accordingly. E.g.:
        //
        //   python utils/extract_tile_rio_tiler.py example_data/example_1_cog_3857_nocompress.tif 20 549687 365589
        //
        //   crate::ppm::write_to_ppm("_test_img.ppm", &tile_data.img).unwrap();
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }

    #[tokio::test]
    async fn test_extract_tile_minio_private() {
        // This is a file in a private bucket, so this is covering our AWS signature v4 authentication
        let mut cog = crate::COG::open("/vsis3/private/example_1_cog_3857_nocompress.tif")
            .await
            .unwrap();
        let tile_data = super::extract_tile(&mut cog, TMSTileCoords::from_zxy(20, 549687, 365589))
            .await
            .unwrap();
        // Expected output should be the same as the `_public` version of this test, so see above for
        // instructions on how to update it
        let expected = crate::ppm::read_ppm(
            "example_data/tests_expected/example_1_cog_3857_nocompress__20_549687_365589.ppm",
        )
        .unwrap();
        assert_eq!(expected.width, 256);
        assert_eq!(expected.height, 256);
        assert_eq!(tile_data.img.data, expected.data);
    }
}
