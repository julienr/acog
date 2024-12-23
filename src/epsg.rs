use crate::Error;

/// Utilities related to EPSG
pub mod spheroid_3857 {
    // According to the spheroid used by 3857, see https://epsg.io/3857
    pub const EARTH_RADIUS_METERS: f64 = 6378137.0;
    pub const EARTH_EQUATOR_CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * EARTH_RADIUS_METERS;
    // That's the "projected bounds" top left
    pub const TOP_LEFT_METERS: (f64, f64) = (
        -EARTH_EQUATOR_CIRCUMFERENCE / 2.0,
        -EARTH_EQUATOR_CIRCUMFERENCE / 2.0,
    );
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UnitOfMeasure {
    LinearMeter, // https://epsg.io/9001-units
    Degree,      // https://epsg.io/9102-units
}

impl UnitOfMeasure {
    pub fn decode(v: u16) -> Result<UnitOfMeasure, Error> {
        match v {
            9001 => Ok(UnitOfMeasure::LinearMeter),
            9102 => Ok(UnitOfMeasure::Degree),
            v => Err(Error::UnsupportedUnit(format!("{}", v))),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Crs {
    PseudoMercator,
    Unknown(u16),
}

impl Crs {
    pub fn decode(v: u16) -> Crs {
        match v {
            3857 => Crs::PseudoMercator,
            v => Crs::Unknown(v),
        }
    }

    pub fn epsg_code(&self) -> u16 {
        match self {
            Crs::PseudoMercator => 3857,
            Crs::Unknown(v) => *v,
        }
    }
}
