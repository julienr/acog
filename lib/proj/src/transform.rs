use std::ffi::CString;
use std::ptr::null_mut;

use crate::bindings;
use crate::context::Context;
use crate::error::Error;

pub struct Transform {
    _context: Context,
    projection: *mut bindings::PJ,
}

pub type Coordinate = (f64, f64);

impl Transform {
    // `from_epsg` and `to_epsg` should be EPSG identifiers like 4326 or 3857
    pub fn new(from_epsg: u16, to_epsg: u16) -> Result<Transform, Error> {
        let mut context = Context::new();
        let c_source_crs = CString::new(format!("EPSG:{}", from_epsg))?;
        let c_target_crs = CString::new(format!("EPSG:{}", to_epsg))?;
        // TODO: Create PJ_AREA using input raster bbox and output tile bbox
        // https://proj.org/en/9.3/development/reference/functions.html#c.proj_area_create
        // => This should lead to more precise transforms when there can be ambiguity
        let proj = unsafe {
            bindings::proj_create_crs_to_crs(
                context.c_ptr(),
                c_source_crs.as_ptr(),
                c_target_crs.as_ptr(),
                null_mut(),
            )
        };
        if proj.is_null() {
            return Err(context.get_error());
        }
        // Ensure we always use longitude, latitude axis order and not the CRS-defined one
        let norm_proj =
            unsafe { bindings::proj_normalize_for_visualization(context.c_ptr(), proj) };
        if norm_proj.is_null() {
            return Err(context.get_error());
        }
        unsafe { bindings::proj_destroy(proj) };
        Ok(Transform {
            _context: context,
            projection: norm_proj,
        })
    }

    pub fn transform(&self, point: Coordinate) -> Coordinate {
        let a = unsafe { bindings::proj_coord(point.0, point.1, 0.0, 0.0) };
        let b = unsafe { bindings::proj_trans(self.projection, bindings::PJ_DIRECTION_PJ_FWD, a) };
        unsafe { (b.xy.x, b.xy.y) }
    }

    pub fn inverse_transform(&self, point: Coordinate) -> Coordinate {
        let a = unsafe { bindings::proj_coord(point.0, point.1, 0.0, 0.0) };
        let b = unsafe { bindings::proj_trans(self.projection, bindings::PJ_DIRECTION_PJ_INV, a) };
        unsafe { (b.xy.x, b.xy.y) }
    }
}

impl Drop for Transform {
    fn drop(&mut self) {
        unsafe { bindings::proj_destroy(self.projection) };
    }
}

#[cfg(test)]
mod tests {
    use testutils::assert_float_eq;

    use super::*;
    /// The point of the tests below is not to retest proj, but mostly to sanity check
    /// that the bindings are working corectly
    ///
    /// Handy tools to generate tests:
    /// https://epsg.io/transform#s_srs=4326&t_srs=3857&x=NaN&y=NaN

    fn assert_coordinate_eq(c1: Coordinate, c2: Coordinate, epsilon: f64) {
        assert_float_eq(c1.0, c2.0, epsilon);
        assert_float_eq(c1.1, c2.1, epsilon);
    }

    #[test]
    fn test_transform_4326_4326() {
        let t = Transform::new(4326, 4326).unwrap();
        let v0 = (42.0, -43.0);
        let v1 = t.transform(v0);
        assert_float_eq(v1.0, 42.0, 1e-5);
        assert_float_eq(v1.1, -43.0, 1e-5);
        let v2 = t.inverse_transform(v1);
        assert_coordinate_eq(v2, v0, 1e-5);
    }

    #[test]
    fn test_transform_4326_3857() {
        // https://epsg.io/transform#s_srs=4326&t_srs=3857&x=42.0000000&y=-43.0000000
        let t = Transform::new(4326, 3857).unwrap();
        let v0 = (42.0, -43.0);
        let v1 = t.transform(v0);
        assert_float_eq(v1.0, 4675418.613317491, 1e-5);
        assert_float_eq(v1.1, -5311971.846945472, 1e-5);
        let v2 = t.inverse_transform(v1);
        assert_coordinate_eq(v2, v0, 1e-5);
    }
}
