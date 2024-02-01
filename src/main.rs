use std::io::SeekFrom;
use std::mem::size_of;
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

fn decode_u8(buf: [u8; 1], _byte_order: ByteOrder) -> u8 {
    u8::from_ne_bytes(buf)
}

fn decode_u16(buf: [u8; 2], byte_order: ByteOrder) -> u16 {
    match byte_order {
        ByteOrder::LittleEndian => u16::from_le_bytes(buf),
        ByteOrder::BigEndian => u16::from_be_bytes(buf),
    }
}

fn decode_u32(buf: [u8; 4], byte_order: ByteOrder) -> u32 {
    match byte_order {
        ByteOrder::LittleEndian => u32::from_le_bytes(buf),
        ByteOrder::BigEndian => u32::from_be_bytes(buf),
    }
}

fn decode_u32_pair(buf: [u8; 8], byte_order: ByteOrder) -> (u32, u32) {
    (
        decode_u32([buf[0], buf[1], buf[2], buf[3]], byte_order),
        decode_u32([buf[4], buf[5], buf[6], buf[7]], byte_order),
    )
}

fn decode_i8(buf: [u8; 1], _byte_order: ByteOrder) -> i8 {
    i8::from_ne_bytes(buf)
}

fn decode_i16(buf: [u8; 2], byte_order: ByteOrder) -> i16 {
    match byte_order {
        ByteOrder::LittleEndian => i16::from_le_bytes(buf),
        ByteOrder::BigEndian => i16::from_be_bytes(buf),
    }
}

fn decode_i32(buf: [u8; 4], byte_order: ByteOrder) -> i32 {
    match byte_order {
        ByteOrder::LittleEndian => i32::from_le_bytes(buf),
        ByteOrder::BigEndian => i32::from_be_bytes(buf),
    }
}

fn decode_i32_pair(buf: [u8; 8], byte_order: ByteOrder) -> (i32, i32) {
    (
        decode_i32([buf[0], buf[1], buf[2], buf[3]], byte_order),
        decode_i32([buf[4], buf[5], buf[6], buf[7]], byte_order),
    )
}

fn decode_f32(buf: [u8; 4], byte_order: ByteOrder) -> f32 {
    match byte_order {
        ByteOrder::LittleEndian => f32::from_le_bytes(buf),
        ByteOrder::BigEndian => f32::from_be_bytes(buf),
    }
}

fn decode_f64(buf: [u8; 8], byte_order: ByteOrder) -> f64 {
    match byte_order {
        ByteOrder::LittleEndian => f64::from_le_bytes(buf),
        ByteOrder::BigEndian => f64::from_be_bytes(buf),
    }
}

fn decode_string(buf: &Vec<u8>, _byte_order: ByteOrder) -> Result<String, Error> {
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

fn decode_vec<T, F, const N: usize>(
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

#[derive(Clone, Copy)]
enum IFDType {
    Byte,
    Ascii,
    Short,
    Long,
    Rational,
    SignedByte,
    SignedShort,
    SignedLong,
    SignedRational,
    Float,
    Double,
    UndefinedRawBytes,
}

fn type_size(ifd_type: IFDType) -> usize {
    match ifd_type {
        IFDType::Byte => 1,
        IFDType::Ascii => 1,
        IFDType::Short => 2,
        IFDType::Long => 4,
        IFDType::Rational => 8,
        IFDType::SignedByte => 1,
        IFDType::SignedShort => 2,
        IFDType::SignedLong => 4,
        IFDType::SignedRational => 8,
        IFDType::Float => 4,
        IFDType::Double => 8,
        IFDType::UndefinedRawBytes => 1,
    }
}

#[derive(Debug, Clone)]
enum IFDValue {
    Byte(Vec<u8>),
    Ascii(String),
    Short(Vec<u16>),
    Long(Vec<u32>),
    Rational(Vec<(u32, u32)>),
    SignedByte(Vec<i8>),
    UndefinedRawBytes(Vec<u8>), // For arbitrary bytes values
    SignedShort(Vec<i16>),
    SignedLong(Vec<i32>),
    SignedRational(Vec<(i32, i32)>),
    Float(Vec<f32>),
    Double(Vec<f64>),
}

#[derive(Debug, Clone)]
enum IFDTag {
    PhotometricInterpretation,
    Compression,
    ImageLength,
    ImageWidth,
    ResolutionUnit,
    XResolution,
    YResolution,
    RowsPerStrip,
    StripOffsets,
    StripByteCounts,
    BitsPerSample,
    Colormap,
    SamplesPerPixel,
    ExtraSamples,
    PlanarConfiguration,
    UnknownTag(u16),
}

fn decode_tag(tag: u16) -> IFDTag {
    match tag {
        262 => IFDTag::PhotometricInterpretation,
        259 => IFDTag::Compression,
        257 => IFDTag::ImageLength,
        256 => IFDTag::ImageWidth,
        296 => IFDTag::ResolutionUnit,
        282 => IFDTag::XResolution,
        283 => IFDTag::YResolution,
        278 => IFDTag::RowsPerStrip,
        273 => IFDTag::StripOffsets,
        279 => IFDTag::StripByteCounts,
        258 => IFDTag::BitsPerSample,
        320 => IFDTag::Colormap,
        277 => IFDTag::SamplesPerPixel,
        338 => IFDTag::ExtraSamples,
        284 => IFDTag::PlanarConfiguration,
        v => IFDTag::UnknownTag(v),
    }
}

#[derive(Debug, Clone)]
struct IFDEntry {
    pub tag: IFDTag,
    pub value: IFDValue,
}

enum OffsetOrInlineValue {
    Offset(u32),
    InlineValue([u8; 4]),
}

struct IFDEntryMetadata {
    pub tag: u16,
    pub field_type: IFDType,
    pub count: u32,
    pub offset: OffsetOrInlineValue,
}

enum RawEntryResult {
    KnownType(IFDEntryMetadata),
    UnknownType(u16),
    InvalidCount(u32),
}

impl IFDEntryMetadata {
    pub async fn read(file: &mut File, byte_order: ByteOrder) -> Result<RawEntryResult, Error> {
        // TODO: Read whole bytes chunk and then parse instead of reading one by one ?
        let mut buf = [0u8; 12];
        file.read_exact(&mut buf).await?;
        let tag = decode_u16([buf[0], buf[1]], byte_order);
        let field_type = decode_u16([buf[2], buf[3]], byte_order);
        let field_type = match field_type {
            0 => return Ok(RawEntryResult::UnknownType(0)),
            v @ 13.. => return Ok(RawEntryResult::UnknownType(v)),
            1 => IFDType::Byte,
            2 => IFDType::Ascii,
            3 => IFDType::Short,
            4 => IFDType::Long,
            5 => IFDType::Rational,
            6 => IFDType::SignedByte,
            7 => IFDType::UndefinedRawBytes,
            8 => IFDType::SignedShort,
            9 => IFDType::SignedLong,
            10 => IFDType::SignedRational,
            11 => IFDType::Float,
            12 => IFDType::Double,
        };
        let count = decode_u32([buf[4], buf[5], buf[6], buf[7]], byte_order);
        if count == 0 {
            return Ok(RawEntryResult::InvalidCount(count));
        }
        let offset: OffsetOrInlineValue = if type_size(field_type) * count as usize <= 4 {
            OffsetOrInlineValue::InlineValue([buf[8], buf[9], buf[10], buf[11]])
        } else {
            OffsetOrInlineValue::Offset(decode_u32([buf[8], buf[9], buf[10], buf[11]], byte_order))
        };
        Ok(RawEntryResult::KnownType(IFDEntryMetadata {
            tag,
            field_type,
            count,
            offset,
        }))
    }

    pub async fn full_read(
        &self,
        file: &mut File,
        byte_order: ByteOrder,
    ) -> Result<IFDEntry, Error> {
        let data = match self.offset {
            OffsetOrInlineValue::InlineValue(arr) => {
                arr[0..type_size(self.field_type) * self.count as usize].to_vec()
            }
            OffsetOrInlineValue::Offset(offset) => {
                let mut data = vec![0u8; type_size(self.field_type) * self.count as usize];
                file.seek(SeekFrom::Start(offset.into())).await?;
                file.read_exact(data.as_mut_slice()).await?;
                data
            }
        };
        let value = match self.field_type {
            IFDType::Byte => IFDValue::Byte(decode_vec(
                &data,
                self.count as usize,
                decode_u8,
                byte_order,
            )),
            IFDType::Ascii => IFDValue::Ascii(decode_string(&data, byte_order)?),
            IFDType::Short => IFDValue::Short(decode_vec(
                &data,
                self.count as usize,
                decode_u16,
                byte_order,
            )),
            IFDType::Long => IFDValue::Long(decode_vec(
                &data,
                self.count as usize,
                decode_u32,
                byte_order,
            )),
            IFDType::Rational => IFDValue::Rational(decode_vec(
                &data,
                self.count as usize,
                decode_u32_pair,
                byte_order,
            )),
            IFDType::SignedByte => IFDValue::SignedByte(decode_vec(
                &data,
                self.count as usize,
                decode_i8,
                byte_order,
            )),
            IFDType::UndefinedRawBytes => IFDValue::UndefinedRawBytes(data),
            IFDType::SignedShort => IFDValue::SignedShort(decode_vec(
                &data,
                self.count as usize,
                decode_i16,
                byte_order,
            )),
            IFDType::SignedLong => IFDValue::SignedLong(decode_vec(
                &data,
                self.count as usize,
                decode_i32,
                byte_order,
            )),
            IFDType::SignedRational => IFDValue::SignedRational(decode_vec(
                &data,
                self.count as usize,
                decode_i32_pair,
                byte_order,
            )),
            IFDType::Float => IFDValue::Float(decode_vec(
                &data,
                self.count as usize,
                decode_f32,
                byte_order,
            )),
            IFDType::Double => IFDValue::Double(decode_vec(
                &data,
                self.count as usize,
                decode_f64,
                byte_order,
            )),
        };
        let tag = decode_tag(self.tag);
        Ok(IFDEntry { tag, value })
    }
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
    let mut entries: Vec<IFDEntryMetadata> = vec![];
    for _ in 0..fields_count {
        match IFDEntryMetadata::read(file, byte_order).await? {
            RawEntryResult::KnownType(e) => entries.push(e),
            RawEntryResult::UnknownType(v) => {
                println!("Unknown tag {:?}", v);
            }
            RawEntryResult::InvalidCount(c) => {
                println!("Invalid count {:?}", c);
            }
        }
    }
    let next_ifd_offset = read_u32(file, byte_order).await?;
    let mut full_entries: Vec<IFDEntry> = vec![];
    for e in entries.iter() {
        full_entries.push(e.full_read(file, byte_order).await?);
    }
    Ok((
        ImageFileDirectory {
            entries: full_entries,
        },
        next_ifd_offset,
    ))
}

#[derive(Debug)]
struct TIFFReader {
    pub byte_order: ByteOrder,
    pub ifds: Vec<ImageFileDirectory>,
    file: File,
}

impl TIFFReader {
    pub async fn open(filename: &str) -> Result<TIFFReader, Error> {
        let mut file = File::open(filename).await?;
        // Byte order & magic number check
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
        let magic_number = read_u16(&mut file, byte_order).await?;
        if magic_number != 42 {
            return Err(Error::InvalidData(format!(
                "Invalid magic_number {:?}",
                magic_number
            )));
        }

        // Read ifds
        let ifds: Vec<ImageFileDirectory> = {
            let mut ifds = vec![];
            let mut ifd_offset = read_u32(&mut file, byte_order).await?;
            // TODO: Infinite loop detection ?
            while ifd_offset > 0 {
                file.seek(SeekFrom::Start(ifd_offset.into())).await?;
                let (ifd, next_ifd_offset) =
                    read_image_file_directory(&mut file, byte_order).await?;
                ifd_offset = next_ifd_offset;
                ifds.push(ifd);
            }
            ifds
        };

        Ok(TIFFReader {
            byte_order,
            ifds,
            file,
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let reader = TIFFReader::open("example_data/example_1_no_compress.tif").await?;
    println!("reader: {:?}", reader);
    Ok(())
}
