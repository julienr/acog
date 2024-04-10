#[derive(Copy, Clone, Debug)]
pub struct Vec2<T> {
    pub x: T,
    pub y: T,
}

pub type Vec2f = Vec2<f64>;

pub fn vec2f(x: f64, y: f64) -> Vec2f {
    Vec2 { x, y }
}

impl<T> From<(T, T)> for Vec2<T> {
    fn from(value: (T, T)) -> Self {
        Vec2 {
            x: value.0,
            y: value.1,
        }
    }
}

impl<T: std::ops::Sub<Output = T> + std::marker::Copy> std::ops::Sub for Vec2<T> {
    type Output = Self;

    fn sub(self, rhs: Vec2<T>) -> Self::Output {
        Vec2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl<T: std::ops::Add<Output = T>> std::ops::Add for Vec2<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Vec2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: std::ops::Mul<Output = T> + std::marker::Copy> std::ops::Mul<T> for Vec2<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Vec2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use testutils::assert_float_eq;

    use super::*;

    fn assert_vec2f_eq(v1: Vec2f, v2: Vec2f, epsilon: f64) {
        assert_float_eq(v1.x, v2.x, epsilon);
        assert_float_eq(v1.y, v2.y, epsilon);
    }

    #[test]
    fn test_vec2_sub() {
        assert_vec2f_eq(
            vec2f(14.0, 32.0) - vec2f(4.0, 40.0),
            vec2f(10.0, -8.0),
            1e-5,
        );
    }

    #[test]
    fn test_vec2_add() {
        assert_vec2f_eq(vec2f(5.0, 7.0) + vec2f(-2.0, 3.0), vec2f(3.0, 10.0), 1e-5);
    }

    #[test]
    fn test_vec2_mul() {
        assert_vec2f_eq(vec2f(5.0, 7.0) * 1.5, vec2f(7.5, 10.5), 1e-5);
    }
}
