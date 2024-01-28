use std::io::SeekFrom;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncSeekExt};

#[derive(Debug, Clone, Copy)]
enum ByteOrder {
    LittleEndian,
    BigEndian,
}

#[derive(Debug)]
enum Error {
    IOError(io::Error),
    InvalidData(String),
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error::IOError(value)
    }
}

async fn read_u16(file: &mut File, byte_order: ByteOrder) -> Result<u16, io::Error> {
    match byte_order {
        ByteOrder::LittleEndian => file.read_u16_le().await,
        ByteOrder::BigEndian => file.read_u16().await,
    }
}

async fn read_u32(file: &mut File, byte_order: ByteOrder) -> Result<u32, io::Error> {
    match byte_order {
        ByteOrder::LittleEndian => file.read_u32_le().await,
        ByteOrder::BigEndian => file.read_u32().await,
    }
}

#[derive(Debug, Clone, Copy)]
enum FieldType {
    Byte,
    Ascii,
    Short,
    Long,
    Rational,
    SignedByte,
    Undefined, // For arbitrary bytes values
    SignedShort,
    SignedLong,
    SignedRational,
    Float,
    Double,
    Unknown(u16),
}

#[derive(Debug)]
struct IFDEntry {
    pub tag: u16,
    pub field_type: FieldType,
    pub count: u32,
    pub offset: u32,
}

async fn read_ifd_entry(file: &mut File, byte_order: ByteOrder) -> Result<IFDEntry, Error> {
    let tag = read_u16(file, byte_order).await?;
    let field_type = match read_u16(file, byte_order).await? {
        0 => FieldType::Unknown(0),
        1 => FieldType::Byte,
        2 => FieldType::Ascii,
        3 => FieldType::Short,
        4 => FieldType::Long,
        5 => FieldType::Rational,
        6 => FieldType::SignedByte,
        7 => FieldType::Undefined,
        8 => FieldType::SignedShort,
        9 => FieldType::SignedLong,
        10 => FieldType::SignedRational,
        11 => FieldType::Float,
        12 => FieldType::Double,
        v => FieldType::Unknown(v),
    };
    let count = read_u32(file, byte_order).await?;
    let offset = read_u32(file, byte_order).await?;
    Ok(IFDEntry {
        tag,
        field_type,
        count,
        offset,
    })
}

#[derive(Debug)]
struct ImageFileDirectory {
    pub entries: Vec<IFDEntry>,
}

async fn read_image_file_directory(
    file: &mut File,
    byte_order: ByteOrder,
) -> Result<(ImageFileDirectory, u32), Error> {
    let fields_count = read_u16(file, byte_order).await?;
    let mut entries = vec![];
    for _ in 0..fields_count {
        entries.push(read_ifd_entry(file, byte_order).await?);
    }
    let next_ifd_offset = read_u32(file, byte_order).await?;
    Ok((ImageFileDirectory { entries }, next_ifd_offset))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let mut file = File::open("example_data/example_1_no_compress.tif").await?;
    let byte_order: ByteOrder = {
        let mut buf = [0u8; 2];
        file.read_exact(&mut buf[..]).await?;
        if buf[0] == 0x49 && buf[1] == 0x49 {
            Ok(ByteOrder::LittleEndian)
        } else if buf[0] == 0x4D && buf[1] == 0x4D {
            Ok(ByteOrder::BigEndian)
        } else {
            Err(Error::InvalidData(format!("Invalid byte_order {:?}", buf)))
        }
    }?;
    println!("byte_order {:?}", byte_order);
    let magic_number = read_u16(&mut file, byte_order).await?;
    if magic_number != 42 {
        return Err(Error::InvalidData(format!(
            "Invalid magic_number {:?}",
            magic_number
        )));
    }
    println!("magic_number {:?}", magic_number);
    let ifd_offset = read_u32(&mut file, byte_order).await?;
    file.seek(SeekFrom::Start(ifd_offset.into())).await?;
    let (ifd, next_ifd_offset) = read_image_file_directory(&mut file, byte_order).await?;
    println!("ifd: {:?}", ifd);
    println!("next_ifd_offset: {:?}", next_ifd_offset);
    Ok(())
}
