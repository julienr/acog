use std::io::{self, Write};

/// Small utility functions to write .npy files. Handy to debug things using python

pub fn write_to_npy(
    filename: &str,
    img_data: Vec<u8>,
    img_shape: [usize; 3],
) -> Result<(), io::Error> {
    let mut file = std::fs::File::create(filename)?;
    let dtype = "'uint8'";
    let dict = format!(
        "{{\"descr\": {}, \"fortran_order\": False, \"shape\": ({}, {}, {})}}\n",
        dtype, img_shape[0], img_shape[1], img_shape[2]
    );
    let dict_bytes = dict.as_bytes();
    let magic = [0x93u8, b'N', b'U', b'M', b'P', b'Y', 0x01, 0x00];
    let size = magic.len() + 2 + dict_bytes.len();
    let padding = 64 * ((size + 63) / 64) - size;
    println!("padding with {} {}", padding, padding + size);
    let header_len = (dict_bytes.len() as u16 + padding as u16).to_le_bytes();
    file.write_all(&magic)?;
    file.write_all(&header_len)?;
    file.write_all(dict_bytes)?;
    file.write_all(&vec![0x20; padding])?;
    file.write_all(&img_data)?;
    file.flush()?;
    Ok(())
}
