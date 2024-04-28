use crate::errors::Error;
/// Low-level byte conversion functions
use std::mem::size_of;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

pub fn decode_u8(buf: [u8; 1], _byte_order: ByteOrder) -> u8 {
    u8::from_ne_bytes(buf)
}

pub fn decode_u16(buf: [u8; 2], byte_order: ByteOrder) -> u16 {
    match byte_order {
        ByteOrder::LittleEndian => u16::from_le_bytes(buf),
        ByteOrder::BigEndian => u16::from_be_bytes(buf),
    }
}

pub fn decode_u32(buf: [u8; 4], byte_order: ByteOrder) -> u32 {
    match byte_order {
        ByteOrder::LittleEndian => u32::from_le_bytes(buf),
        ByteOrder::BigEndian => u32::from_be_bytes(buf),
    }
}

pub fn decode_u32_from_slice(buf: &[u8], byte_order: ByteOrder) -> u32 {
    let mut data = [0u8; 4];
    data.copy_from_slice(buf);
    decode_u32(data, byte_order)
}

pub fn decode_u64(buf: [u8; 8], byte_order: ByteOrder) -> u64 {
    match byte_order {
        ByteOrder::LittleEndian => u64::from_le_bytes(buf),
        ByteOrder::BigEndian => u64::from_be_bytes(buf),
    }
}

pub fn decode_u64_from_slice(buf: &[u8], byte_order: ByteOrder) -> u64 {
    let mut data = [0u8; 8];
    data.copy_from_slice(buf);
    decode_u64(data, byte_order)
}

pub fn decode_u32_pair(buf: [u8; 8], byte_order: ByteOrder) -> (u32, u32) {
    (
        decode_u32([buf[0], buf[1], buf[2], buf[3]], byte_order),
        decode_u32([buf[4], buf[5], buf[6], buf[7]], byte_order),
    )
}

pub fn decode_i8(buf: [u8; 1], _byte_order: ByteOrder) -> i8 {
    i8::from_ne_bytes(buf)
}

pub fn decode_i16(buf: [u8; 2], byte_order: ByteOrder) -> i16 {
    match byte_order {
        ByteOrder::LittleEndian => i16::from_le_bytes(buf),
        ByteOrder::BigEndian => i16::from_be_bytes(buf),
    }
}

pub fn decode_i32(buf: [u8; 4], byte_order: ByteOrder) -> i32 {
    match byte_order {
        ByteOrder::LittleEndian => i32::from_le_bytes(buf),
        ByteOrder::BigEndian => i32::from_be_bytes(buf),
    }
}

pub fn decode_i32_pair(buf: [u8; 8], byte_order: ByteOrder) -> (i32, i32) {
    (
        decode_i32([buf[0], buf[1], buf[2], buf[3]], byte_order),
        decode_i32([buf[4], buf[5], buf[6], buf[7]], byte_order),
    )
}

pub fn decode_f32(buf: [u8; 4], byte_order: ByteOrder) -> f32 {
    match byte_order {
        ByteOrder::LittleEndian => f32::from_le_bytes(buf),
        ByteOrder::BigEndian => f32::from_be_bytes(buf),
    }
}

pub fn decode_f64(buf: [u8; 8], byte_order: ByteOrder) -> f64 {
    match byte_order {
        ByteOrder::LittleEndian => f64::from_le_bytes(buf),
        ByteOrder::BigEndian => f64::from_be_bytes(buf),
    }
}

pub fn decode_string(buf: &[u8], _byte_order: ByteOrder) -> Result<String, Error> {
    let mut str: String = "".to_string();
    if buf[buf.len() - 1] != b'\0' {
        return Err(Error::InvalidData(
            "string not terminated by null character".to_string(),
        ));
    }
    for v in &buf[..buf.len() - 1] {
        let ch = char::from_u32(*v as u32);
        match ch {
            None => {
                return Err(Error::InvalidData(format!("invalid character {:?}", v)));
            }
            Some('\0') => {
                return Err(Error::InvalidData(
                    "unexpected EOS character before count".to_string(),
                ))
            }
            Some(c) => str.push(c),
        }
    }

    Ok(str)
}

pub fn decode_vec<T, F, const N: usize>(
    buf: &[u8],
    count: usize,
    decode_fn: F,
    byte_order: ByteOrder,
) -> Vec<T>
where
    F: Fn([u8; N], ByteOrder) -> T,
{
    let mut out = vec![];
    let type_size: usize = size_of::<T>();
    for i in 0..count {
        out.push(decode_fn(
            buf[i * type_size..(i + 1) * type_size].try_into().unwrap(),
            byte_order,
        ))
    }
    out
}
