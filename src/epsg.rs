/// Utilities related to EPSG

#[derive(Debug)]
pub enum UnitOfMeasure {
    LinearMeter, // https://epsg.io/9001-units
    Unknown(u16)
}

impl UnitOfMeasure {
    pub fn decode(v: u16) -> UnitOfMeasure {
        match v {
            9001 => UnitOfMeasure::LinearMeter,
            v => UnitOfMeasure::Unknown(v)
        }
    }
}

#[derive(Debug)]
pub enum Crs {
    PseudoMercator,
    Unknown(u16)
}

impl Crs {
    pub fn decode(v: u16) -> Crs {
        match v {
            3857 => Crs::PseudoMercator,
            v => Crs::Unknown(v)
        }
    }
}