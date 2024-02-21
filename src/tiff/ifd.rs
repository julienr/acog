/// Base functionality to read TIFF IFDs (ImageFileDirectory)
use std::mem::size_of;

use super::low_level::*;
use crate::errors::Error;
use crate::sources::{self, CachedSource, Source};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
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
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub enum IFDValue {
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub enum IFDTag {
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
    TileLength,
    TileWidth,
    TileOffsets,
    TileByteCounts,
    // TODO: See the COG spec for how to handle those values:
    // https://docs.ogc.org/is/21-026/21-026.html
    // 7.2.1. Requirement Reduced-Resolution Subfiles
    NewSubfileType,
    SampleFormat,
    Predictor,
    // Geotiff tags
    ModelPixelScaleTag,
    ModelTiepointTag,
    GeoKeyDirectoryTag,
    GeoDoubleParamsTag,
    GeoAsciiParamsTag,
    // GDAL specific: https://www.awaresystems.be/imaging/tiff/tifftags/gdal_metadata.html
    GdalMetadata,
    UnknownTag(u16),
}

// Those are exposed because they are required by `geo_keys` for parsing GeoKeyDirectory
pub const GEO_DOUBLE_PARAMS_TAG: u16 = 34736;
pub const GEO_ASCII_PARAMS_TAG: u16 = 34737;
pub const GEO_KEY_DIRECTORY_TAG: u16 = 34735;

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
        323 => IFDTag::TileLength,
        322 => IFDTag::TileWidth,
        324 => IFDTag::TileOffsets,
        325 => IFDTag::TileByteCounts,
        254 => IFDTag::NewSubfileType,
        339 => IFDTag::SampleFormat,
        317 => IFDTag::Predictor,
        33550 => IFDTag::ModelPixelScaleTag,
        33922 => IFDTag::ModelTiepointTag,
        GEO_KEY_DIRECTORY_TAG => IFDTag::GeoKeyDirectoryTag,
        GEO_DOUBLE_PARAMS_TAG => IFDTag::GeoDoubleParamsTag,
        GEO_ASCII_PARAMS_TAG => IFDTag::GeoAsciiParamsTag,
        42112 => IFDTag::GdalMetadata,
        v => IFDTag::UnknownTag(v),
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct IFDEntry {
    pub tag: IFDTag,
    pub value: IFDValue,
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
enum OffsetOrInlineValue {
    Offset(u32),
    InlineValue([u8; 4]),
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
struct IFDEntryMetadata {
    pub tag: IFDTag,
    pub field_type: IFDType,
    pub count: u32,
    pub offset: OffsetOrInlineValue,
    byte_order: ByteOrder,
}

#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct FullyDecodedIFDEntry {
    pub tag: IFDTag,
    pub value: IFDValue,
}

enum RawEntryResult {
    KnownType(IFDEntryMetadata),
    UnknownType(u16),
    InvalidCount(u32),
}

async fn read_u16(
    cached_source: &mut CachedSource,
    offset: u64,
    byte_order: ByteOrder,
) -> Result<u16, Error> {
    let mut buf = [0u8; 2];
    cached_source.read_exact(offset, &mut buf).await?;
    Ok(decode_u16(buf, byte_order))
}

async fn read_u32(
    cached_source: &mut CachedSource,
    offset: u64,
    byte_order: ByteOrder,
) -> Result<u32, Error> {
    let mut buf = [0u8; 4];
    cached_source.read_exact(offset, &mut buf).await?;
    Ok(decode_u32(buf, byte_order))
}

impl IFDEntryMetadata {
    pub async fn decode(buf: &[u8; 12], byte_order: ByteOrder) -> Result<RawEntryResult, Error> {
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
            tag: decode_tag(tag),
            field_type,
            count,
            offset,
            byte_order,
        }))
    }

    pub async fn read_value(&self, source: &mut CachedSource) -> Result<IFDValue, Error> {
        let data = match self.offset {
            OffsetOrInlineValue::InlineValue(arr) => {
                arr[0..type_size(self.field_type) * self.count as usize].to_vec()
            }
            OffsetOrInlineValue::Offset(offset) => {
                let mut data = vec![0u8; type_size(self.field_type) * self.count as usize];
                source
                    .read_exact(offset as u64, data.as_mut_slice())
                    .await?;
                data
            }
        };
        let value = match self.field_type {
            IFDType::Byte => IFDValue::Byte(decode_vec(
                &data,
                self.count as usize,
                decode_u8,
                self.byte_order,
            )),
            IFDType::Ascii => IFDValue::Ascii(decode_string(&data, self.byte_order)?),
            IFDType::Short => IFDValue::Short(decode_vec(
                &data,
                self.count as usize,
                decode_u16,
                self.byte_order,
            )),
            IFDType::Long => IFDValue::Long(decode_vec(
                &data,
                self.count as usize,
                decode_u32,
                self.byte_order,
            )),
            IFDType::Rational => IFDValue::Rational(decode_vec(
                &data,
                self.count as usize,
                decode_u32_pair,
                self.byte_order,
            )),
            IFDType::SignedByte => IFDValue::SignedByte(decode_vec(
                &data,
                self.count as usize,
                decode_i8,
                self.byte_order,
            )),
            IFDType::UndefinedRawBytes => IFDValue::UndefinedRawBytes(data),
            IFDType::SignedShort => IFDValue::SignedShort(decode_vec(
                &data,
                self.count as usize,
                decode_i16,
                self.byte_order,
            )),
            IFDType::SignedLong => IFDValue::SignedLong(decode_vec(
                &data,
                self.count as usize,
                decode_i32,
                self.byte_order,
            )),
            IFDType::SignedRational => IFDValue::SignedRational(decode_vec(
                &data,
                self.count as usize,
                decode_i32_pair,
                self.byte_order,
            )),
            IFDType::Float => IFDValue::Float(decode_vec(
                &data,
                self.count as usize,
                decode_f32,
                self.byte_order,
            )),
            IFDType::Double => IFDValue::Double(decode_vec(
                &data,
                self.count as usize,
                decode_f64,
                self.byte_order,
            )),
        };
        Ok(value)
    }

    pub async fn read(&self, source: &mut CachedSource) -> Result<FullyDecodedIFDEntry, Error> {
        Ok(FullyDecodedIFDEntry {
            tag: self.tag,
            value: self.read_value(source).await?,
        })
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct ImageFileDirectory {
    entries: Vec<IFDEntryMetadata>,
}

impl ImageFileDirectory {
    pub async fn get_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<IFDValue, Error> {
        let entry = self.entries.iter().find(|e| e.tag == tag);
        match entry {
            Some(e) => Ok(e.read_value(source).await?),
            None => Err(Error::RequiredTagNotFound(tag)),
        }
    }

    pub async fn get_usize_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<usize, Error> {
        Ok(self.get_vec_usize_tag_value(source, tag).await?[0])
    }

    pub async fn get_vec_usize_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<Vec<usize>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Short(values) => Ok(values.iter().map(|v| *v as usize).collect()),
            IFDValue::Long(values) => Ok(values.iter().map(|v| *v as usize).collect()),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_vec_short_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<Vec<u16>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Short(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_vec_double_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<Vec<f64>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Double(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_string_tag_value(
        &self,
        source: &mut CachedSource,
        tag: IFDTag,
    ) -> Result<String, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Ascii(value) => Ok(value),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }
}

async fn read_image_file_directory(
    cached_source: &mut CachedSource,
    offset: u64,
    byte_order: ByteOrder,
) -> Result<(ImageFileDirectory, u32), Error> {
    let fields_count = read_u16(cached_source, offset, byte_order).await? as u64;
    let offset = offset + size_of::<u16>() as u64;
    let mut entries: Vec<IFDEntryMetadata> = vec![];
    for i in 0..fields_count {
        let mut buf = [0u8; 12];
        cached_source.read_exact(offset + i * 12, &mut buf).await?;
        match IFDEntryMetadata::decode(&buf, byte_order).await? {
            RawEntryResult::KnownType(e) => entries.push(e),
            RawEntryResult::UnknownType(v) => {
                println!("Unknown type {:?}", v);
            }
            RawEntryResult::InvalidCount(c) => {
                println!("Invalid count {:?}", c);
            }
        }
    }
    let next_ifd_offset = read_u32(cached_source, offset + fields_count * 12, byte_order).await?;
    /*
    let mut full_entries: Vec<IFDEntry> = vec![];
    for e in entries.iter() {
        full_entries.push(e.full_read(file, byte_order).await?);
    }
    */
    Ok((ImageFileDirectory { entries }, next_ifd_offset))
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct TIFFReader {
    pub ifds: Vec<ImageFileDirectory>,
    #[cfg_attr(feature = "json", serde(skip_serializing))]
    pub source: CachedSource,
}

impl TIFFReader {
    pub async fn open_from_source_spec(source_spec: &String) -> Result<TIFFReader, Error> {
        let source_string = source_spec.to_string();
        if source_string.starts_with("/vsis3/") {
            let file_source =
                sources::S3Source::new(source_string.strip_prefix("/vsis3/").unwrap()).await?;
            let reader = Self::open_from_source(sources::Source::S3(file_source)).await?;
            Ok(reader)
        } else {
            let file_source = sources::FileSource::new(&source_string).await?;
            let reader = Self::open_from_source(sources::Source::File(file_source)).await?;
            Ok(reader)
        }
    }
    pub async fn open_from_source(source: Source) -> Result<TIFFReader, Error> {
        let mut cached_source = CachedSource::new(source);
        // Byte order & magic number check
        let byte_order: ByteOrder = {
            let mut buf = [0u8; 2];
            cached_source.read_exact(0, &mut buf[..]).await?;
            if buf[0] == 0x49 && buf[1] == 0x49 {
                Ok(ByteOrder::LittleEndian)
            } else if buf[0] == 0x4D && buf[1] == 0x4D {
                Ok(ByteOrder::BigEndian)
            } else {
                Err(Error::InvalidData(format!("Invalid byte_order {:?}", buf)))
            }
        }?;
        let magic_number = read_u16(&mut cached_source, 2, byte_order).await?;
        if magic_number != 42 {
            return Err(Error::InvalidData(format!(
                "Invalid magic_number {:?}",
                magic_number
            )));
        }

        // Read ifds
        let ifds: Vec<ImageFileDirectory> = {
            let mut ifds = vec![];
            let mut ifd_offset = read_u32(&mut cached_source, 4, byte_order).await?;
            // TODO: Infinite loop detection ?
            while ifd_offset > 0 {
                let (ifd, next_ifd_offset) =
                    read_image_file_directory(&mut cached_source, ifd_offset as u64, byte_order)
                        .await?;
                ifd_offset = next_ifd_offset;
                ifds.push(ifd);
            }
            ifds
        };

        Ok(TIFFReader {
            ifds,
            source: cached_source,
        })
    }

    /// This will fully read + decode all ifd entries in the file
    pub async fn fully_read_ifds(&mut self) -> Result<Vec<Vec<FullyDecodedIFDEntry>>, Error> {
        let mut fully_decoded_ifds: Vec<Vec<FullyDecodedIFDEntry>> = vec![];
        for ifd in self.ifds.iter() {
            let mut decoded_entries = vec![];
            for e in ifd.entries.iter() {
                decoded_entries.push(e.read(&mut self.source).await?);
            }
            fully_decoded_ifds.push(decoded_entries);
        }
        Ok(fully_decoded_ifds)
    }

    pub fn print_cache_stats(&self) {
        println!("cache stats: {}", self.source.cache_stats());
    }
}
