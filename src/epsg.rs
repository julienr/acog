/// Utilities related to EPSG

#[derive(Debug, Copy, Clone)]
pub enum UnitOfMeasure {
    LinearMeter, // https://epsg.io/9001-units
    Unknown(u16),
}

impl UnitOfMeasure {
    pub fn decode(v: u16) -> UnitOfMeasure {
        match v {
            9001 => UnitOfMeasure::LinearMeter,
            v => UnitOfMeasure::Unknown(v),
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
