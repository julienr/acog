use crate::math::{vec2f, Vec2f};

#[derive(Debug)]
pub struct BoundingBox {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl BoundingBox {
    /// Return all edges of this bounding box, as a pair of (start vertex, end vertex)
    pub fn edges(&self) -> [(Vec2f, Vec2f); 4] {
        let tl = vec2f(self.xmin, self.ymin);
        let tr = vec2f(self.xmax, self.ymin);
        let br = vec2f(self.xmax, self.ymax);
        let bl = vec2f(self.xmin, self.ymax);
        [(tl, tr), (tr, br), (br, bl), (bl, tl)]
    }

    pub fn from_points(points: &Vec<Vec2f>) -> BoundingBox {
        let mut mins: [f64; 2] = [f64::INFINITY, f64::INFINITY];
        let mut maxs: [f64; 2] = [f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in points {
            mins[0] = f64::min(mins[0], p.x);
            mins[1] = f64::min(mins[1], p.y);
            maxs[0] = f64::max(maxs[0], p.x);
            maxs[1] = f64::max(maxs[1], p.y);
        }
        BoundingBox {
            xmin: mins[0],
            ymin: mins[1],
            xmax: maxs[0],
            ymax: maxs[1],
        }
    }
}
