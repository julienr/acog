pub fn float_eq(v1: f64, v2: f64, epsilon: f64) -> bool {
    let diff = (v1 - v2).abs();
    diff <= epsilon
}

pub fn assert_float_eq(v1: f64, v2: f64, epsilon: f64) {
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
