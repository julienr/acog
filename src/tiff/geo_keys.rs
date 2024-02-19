use super::ifd::{
    IFDTag, ImageFileDirectory, GEO_ASCII_PARAMS_TAG, GEO_DOUBLE_PARAMS_TAG, GEO_KEY_DIRECTORY_TAG,
};
use crate::{sources::CachedSource, Error};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyID {
    GTModelType,
    GTRasterType,
    GTCitation,
    GeodeticCRS,
    GeogCitation,
    GeodeticLinearUnits,
    GeodeticAngularUnits,
    EllipsoidSemiMajorAxis,
    EllipsoidInvFlattening,
    ProjectedCRS,
    ProjLinearUnits,
    UnknownKey(u16),
}

fn decode_key_id(v: u16) -> KeyID {
    match v {
        1024 => KeyID::GTModelType,
        1025 => KeyID::GTRasterType,
        1026 => KeyID::GTCitation,
        2048 => KeyID::GeodeticCRS,
        2049 => KeyID::GeogCitation,
        2052 => KeyID::GeodeticLinearUnits,
        2054 => KeyID::GeodeticAngularUnits,
        2057 => KeyID::EllipsoidSemiMajorAxis,
        2059 => KeyID::EllipsoidInvFlattening,
        3072 => KeyID::ProjectedCRS,
        3076 => KeyID::ProjLinearUnits,
        v => KeyID::UnknownKey(v),
    }
}

#[derive(Debug, Clone)]
pub enum KeyValue {
    Short(Vec<u16>),
    Ascii(String),
    Double(Vec<f64>),
}

#[derive(Debug)]
struct GeoKeyEntry {
    pub id: KeyID,
    pub value: KeyValue,
}

impl GeoKeyEntry {
    async fn decode(
        data: &[u16],
        ifd: &ImageFileDirectory,
        source: &mut CachedSource,
    ) -> Result<GeoKeyEntry, Error> {
        if data.len() < 4 {
            return Err(Error::NotACOG(format!(
                "Trying to decode a geokey from less than 4 shorts: got {}",
                data.len()
            )));
        }
        let id = decode_key_id(data[0]);
        let tiff_tag_location = data[1];
        let count = data[2];
        let value_offset = data[3];
        let value: KeyValue = match tiff_tag_location {
            0 => {
                if count != 1 {
                    return Err(Error::NotACOG(format!(
                        "Got TIFFTagLocation=0, but count != 1, got {}",
                        count
                    )));
                }
                KeyValue::Short(vec![value_offset])
            }
            GEO_DOUBLE_PARAMS_TAG => {
                let values = ifd
                    .get_vec_double_tag_value(source, IFDTag::GeoDoubleParamsTag)
                    .await?;
                let end = value_offset as usize + count as usize;
                if value_offset as usize > values.len() || end > values.len() {
                    return Err(Error::NotACOG(format!(
                        "Out of bounds read on GeoDoubleParamsTag, got range {} to {}, len is {}",
                        value_offset,
                        end,
                        values.len()
                    )));
                }
                KeyValue::Double(values[value_offset as usize..end].to_vec())
            }
            GEO_ASCII_PARAMS_TAG => {
                // The spec is a bit unclear whether 'count' should be used here, but in practice
                // it looks like as for TIFF tags, the value_offset and count are to be interpreted as
                // characters
                let values = ifd
                    .get_string_tag_value(source, IFDTag::GeoAsciiParamsTag)
                    .await?;
                if value_offset as usize > values.len()
                    || (value_offset + count) as usize > values.len()
                {
                    return Err(Error::NotACOG(format!(
                        "Out of bounds read on GeoAsciiParamsTag, got value_offset={}, count={}, len is {}",
                        value_offset,
                        count,
                        values.len()
                    )));
                }
                // GeoTIFF uses '|' as the delimiter (instead of \0) for reasons explained in the
                // "Note on ASCII Keys." comment of section B.1.4 of the GeoTIFF spec. We strip the ending
                // | here
                let mut val =
                    values[value_offset as usize..(value_offset + count) as usize].to_string();
                val = match val.strip_suffix('|') {
                    Some(v) => v.to_string(),
                    None => {
                        return Err(Error::NotACOG(format!(
                            "Expected | to separate strings, but didn't get it in val={}",
                            val
                        )));
                    }
                };
                KeyValue::Ascii(val)
            }
            GEO_KEY_DIRECTORY_TAG => {
                // Arrays of short will be placed at the end of the geo key directory tag array
                let values = ifd
                    .get_vec_short_tag_value(source, IFDTag::GeoKeyDirectoryTag)
                    .await?;
                let end = value_offset as usize + count as usize;
                if value_offset as usize > values.len() || end > values.len() {
                    return Err(Error::NotACOG(format!(
                        "Out of bounds read on GeoKeyDirectoryTag, got range {} to {}, len is {}",
                        value_offset,
                        end,
                        values.len()
                    )));
                }
                KeyValue::Short(values[value_offset as usize..end].to_vec())
            }
            v => {
                return Err(Error::NotACOG(format!(
                    "Got invalid TIFFTagLocation: {}",
                    v
                )))
            }
        };
        Ok(GeoKeyEntry { id, value })
    }
}

#[derive(Debug)]
pub struct GeoKeyDirectory {
    keys: Vec<GeoKeyEntry>,
}

impl GeoKeyDirectory {
    pub fn get_key_value(&self, id: KeyID) -> Result<&KeyValue, Error> {
        let entry = self.keys.iter().find(|e| e.id == id);
        match entry {
            Some(e) => Ok(&e.value),
            None => Err(Error::RequiredGeoKeyNotFound(id)),
        }
    }

    pub fn get_vec_short_key_value(&self, id: KeyID) -> Result<&Vec<u16>, Error> {
        match self.get_key_value(id)? {
            KeyValue::Short(values) => Ok(values),
            value => Err(Error::GeoKeyHasWrongType(id, value.clone())),
        }
    }

    pub fn get_short_key_value(&self, id: KeyID) -> Result<u16, Error> {
        Ok(self.get_vec_short_key_value(id)?[0])
    }

    pub async fn from_ifd(
        ifd: &ImageFileDirectory,
        source: &mut CachedSource,
    ) -> Result<GeoKeyDirectory, Error> {
        let directory = ifd
            .get_vec_short_tag_value(source, IFDTag::GeoKeyDirectoryTag)
            .await?;
        // Header len check
        if directory.len() < 4 {
            return Err(Error::NotACOG(format!(
                "GeoKeyDirectoryTag len < 4: {}",
                directory.len(),
            )));
        }
        // Version check
        {
            let version = directory[0];
            if version != 1 {
                return Err(Error::NotACOG(format!(
                    "Unsupported GeoKeyDirectoryTag version. Expected 1, got {}",
                    version
                )));
            }
        }
        // Revision (major + minor)
        {
            let revision = directory[1];
            if revision != 1 {
                return Err(Error::NotACOG(format!(
                    "Unsupported GeoKeyDirectoryTag revision. Expected 1, got {}",
                    revision
                )));
            }
            let minor = directory[2];
            if minor != 0 && minor != 1 {
                return Err(Error::NotACOG(format!(
                    "Unsupported GeoKeyDirectoryTag minor revision. Expected 0 or 1, got {}",
                    minor
                )));
            }
        }
        // Number of keys
        let keys_count = directory[3] as usize;
        {
            let expected_min_len = 4 + keys_count * 4;
            if directory.len() < expected_min_len {
                return Err(Error::NotACOG(format!(
                    "GeoKeyDirectoryTag keys_count={}, so expected a min len of {}; got {}",
                    keys_count,
                    expected_min_len,
                    directory.len()
                )));
            }
        }
        let mut keys = vec![];
        for i in 0..keys_count {
            let key_data = &directory[4 + i * 4..4 + (i + 1) * 4];
            keys.push(GeoKeyEntry::decode(key_data, ifd, source).await?);
        }
        Ok(GeoKeyDirectory { keys })
    }
}
