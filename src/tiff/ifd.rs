/// Base functionality to read TIFF IFDs (ImageFileDirectory)
use std::mem::size_of;

use super::low_level::*;
use crate::errors::Error;
use crate::sources::Source;

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
    Unsigned64,
    Signed64,
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
        IFDType::Unsigned64 => 8,
        IFDType::Signed64 => 8,
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
    Unsigned64(Vec<u64>),
    Signed64(Vec<u64>),
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
    Orientation,
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
    // JPEG tables - see GDAL: https://github.com/OSGeo/gdal/blob/7d3e653b5ed80f281d8664ee4bb217b24d9980bf/frmts/gtiff/libtiff/tiff.h#L345C9-L345C27
    JpegTables,
    YCbCrSubSampling,
    ReferenceBlackWhite,
    UnknownTag(u16),
}

// Those are exposed because they are required by `geo_keys` for parsing GeoKeyDirectory
pub const GEO_DOUBLE_PARAMS_TAG: u16 = 34736;
pub const GEO_ASCII_PARAMS_TAG: u16 = 34737;
pub const GEO_KEY_DIRECTORY_TAG: u16 = 34735;

// See also GDAL header for reference (TIFFTAG_XXX defines)
// https://github.com/OSGeo/gdal/blob/7d3e653b5ed80f281d8664ee4bb217b24d9980bf/frmts/gtiff/libtiff/tiff.h
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
        274 => IFDTag::Orientation,
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
        // "JPEGTables field" in
        // https://download.osgeo.org/libtiff/old/TTN2.draft.txt
        347 => IFDTag::JpegTables,
        317 => IFDTag::Predictor,
        530 => IFDTag::YCbCrSubSampling,
        532 => IFDTag::ReferenceBlackWhite,
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
    Offset(u64),
    FourBytesInlineValue([u8; 4]),
    EightBytesInlineValue([u8; 8]),
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
struct IFDEntryMetadata {
    pub tag: IFDTag,
    pub field_type: IFDType,
    pub count: u64,
    pub offset_or_value: OffsetOrInlineValue,
}

#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct FullyDecodedIFDEntry {
    pub tag: IFDTag,
    pub value: IFDValue,
}

enum RawEntryResult {
    KnownType(IFDEntryMetadata),
    // This will contain (tag, type)
    UnknownType(IFDTag, u16),
    // This will contain (tag, count)
    InvalidCount(IFDTag, u64),
}

async fn read_u16(source: &mut Source, offset: u64) -> Result<u16, Error> {
    let mut buf = [0u8; 2];
    source.read_exact(offset, &mut buf).await?;
    Ok(decode_u16(buf))
}

async fn read_u32(source: &mut Source, offset: u64) -> Result<u32, Error> {
    let mut buf = [0u8; 4];
    source.read_exact(offset, &mut buf).await?;
    Ok(decode_u32(buf))
}

async fn read_u64(source: &mut Source, offset: u64) -> Result<u64, Error> {
    let mut buf = [0u8; 8];
    source.read_exact(offset, &mut buf).await?;
    Ok(decode_u64(buf))
}

impl IFDEntryMetadata {
    pub async fn read_value(&self, source: &mut Source) -> Result<IFDValue, Error> {
        let data = match self.offset_or_value {
            OffsetOrInlineValue::FourBytesInlineValue(arr) => {
                arr[0..type_size(self.field_type) * self.count as usize].to_vec()
            }
            OffsetOrInlineValue::EightBytesInlineValue(arr) => {
                arr[0..type_size(self.field_type) * self.count as usize].to_vec()
            }
            OffsetOrInlineValue::Offset(offset) => {
                let mut data = vec![0u8; type_size(self.field_type) * self.count as usize];
                source.read_exact(offset, data.as_mut_slice()).await?;
                data
            }
        };
        let value = match self.field_type {
            IFDType::Byte => IFDValue::Byte(decode_vec(&data, self.count as usize, decode_u8)),
            IFDType::Ascii => IFDValue::Ascii(decode_string(&data)?),
            IFDType::Short => IFDValue::Short(decode_vec(&data, self.count as usize, decode_u16)),
            IFDType::Long => IFDValue::Long(decode_vec(&data, self.count as usize, decode_u32)),
            IFDType::Rational => {
                IFDValue::Rational(decode_vec(&data, self.count as usize, decode_u32_pair))
            }
            IFDType::SignedByte => {
                IFDValue::SignedByte(decode_vec(&data, self.count as usize, decode_i8))
            }
            IFDType::UndefinedRawBytes => IFDValue::UndefinedRawBytes(data),
            IFDType::SignedShort => {
                IFDValue::SignedShort(decode_vec(&data, self.count as usize, decode_i16))
            }
            IFDType::SignedLong => {
                IFDValue::SignedLong(decode_vec(&data, self.count as usize, decode_i32))
            }
            IFDType::SignedRational => {
                IFDValue::SignedRational(decode_vec(&data, self.count as usize, decode_i32_pair))
            }
            IFDType::Float => IFDValue::Float(decode_vec(&data, self.count as usize, decode_f32)),
            IFDType::Double => IFDValue::Double(decode_vec(&data, self.count as usize, decode_f64)),
            IFDType::Unsigned64 => {
                IFDValue::Unsigned64(decode_vec(&data, self.count as usize, decode_u64))
            }
            IFDType::Signed64 => {
                IFDValue::Signed64(decode_vec(&data, self.count as usize, decode_u64))
            }
        };
        Ok(value)
    }

    pub async fn read(&self, source: &mut Source) -> Result<FullyDecodedIFDEntry, Error> {
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
    pub async fn get_tag_value(&self, source: &mut Source, tag: IFDTag) -> Result<IFDValue, Error> {
        let entry = self.entries.iter().find(|e| e.tag == tag);
        match entry {
            Some(e) => Ok(e.read_value(source).await?),
            None => Err(Error::RequiredTagNotFound(tag)),
        }
    }

    pub async fn get_u64_tag_value(&self, source: &mut Source, tag: IFDTag) -> Result<u64, Error> {
        Ok(self.get_vec_u64_tag_value(source, tag).await?[0])
    }

    pub async fn get_vec_u64_tag_value(
        &self,
        source: &mut Source,
        tag: IFDTag,
    ) -> Result<Vec<u64>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Short(values) => Ok(values.iter().map(|v| *v as u64).collect()),
            IFDValue::Long(values) => Ok(values.iter().map(|v| *v as u64).collect()),
            IFDValue::Unsigned64(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_vec_short_tag_value(
        &self,
        source: &mut Source,
        tag: IFDTag,
    ) -> Result<Vec<u16>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Short(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_vec_double_tag_value(
        &self,
        source: &mut Source,
        tag: IFDTag,
    ) -> Result<Vec<f64>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Double(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_vec_undefined_raw_byte_tag_value(
        &self,
        source: &mut Source,
        tag: IFDTag,
    ) -> Result<Vec<u8>, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::UndefinedRawBytes(values) => Ok(values),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }

    pub async fn get_string_tag_value(
        &self,
        source: &mut Source,
        tag: IFDTag,
    ) -> Result<String, Error> {
        match self.get_tag_value(source, tag).await? {
            IFDValue::Ascii(value) => Ok(value),
            value => Err(Error::TagHasWrongType(tag, value)),
        }
    }
}

// We support both classic and BigTIFF files.
// The main differences between them is whether fields have 32 or 64 bits size. The strategy we adopt
// is that all our data structures are tailored for BigTIFF (e.g. 64 bits offsets) and it's easy to
// turn classic TIFF into that.
// This enum contains a bunch of 'reading' function that will abstract this away
enum TIFFVariant {
    // The TIFF standard: http://download.osgeo.org/geotiff/spec/tiff6.pdf
    Classic,
    // The BigTIFF de facto standard: https://www.awaresystems.be/imaging/tiff/bigtiff.html
    BigTiff,
}

impl TIFFVariant {
    async fn read_initial_ifd_offset(&self, source: &mut Source) -> Result<u64, Error> {
        match self {
            TIFFVariant::Classic => Ok(read_u32(source, 4).await? as u64),
            TIFFVariant::BigTiff => {
                let offset_bytesize = read_u16(source, 4).await?;
                if offset_bytesize != 8 {
                    return Err(Error::InvalidData(format!(
                        "Invalid offset bytesize {}",
                        offset_bytesize
                    )));
                }
                let pad = read_u16(source, 6).await?;
                if pad != 0 {
                    return Err(Error::InvalidData(format!("Invalid pad {}", pad)));
                }
                Ok(read_u64(source, 8).await?)
            }
        }
    }

    fn ifd_entry_size(&self) -> usize {
        match self {
            TIFFVariant::Classic => 12,
            TIFFVariant::BigTiff => 20,
        }
    }

    fn ifd_offset_size(&self) -> usize {
        match self {
            TIFFVariant::Classic => 4,
            TIFFVariant::BigTiff => 8,
        }
    }

    async fn read_image_file_directory(
        &self,
        source: &mut Source,
        offset: u64,
    ) -> Result<(ImageFileDirectory, u64), Error> {
        let (fields_count, offset) = match self {
            TIFFVariant::Classic => (
                read_u16(source, offset).await? as usize,
                offset + size_of::<u16>() as u64,
            ),
            TIFFVariant::BigTiff => (
                read_u64(source, offset).await? as usize,
                offset + size_of::<u64>() as u64,
            ),
        };
        // Read the ifd fields info + next ifd offset all at once
        let mut ifd_data = vec![0u8; fields_count * self.ifd_entry_size() + self.ifd_offset_size()];
        source.read_exact(offset, &mut ifd_data).await?;
        let mut entries: Vec<IFDEntryMetadata> = vec![];
        for i in 0..fields_count {
            let entry_start = i * self.ifd_entry_size();
            let entry_end = (i + 1) * self.ifd_entry_size();
            let buf: &[u8] = &ifd_data[entry_start..entry_end];
            match self.decode_ifd_entry_metadata(buf).await? {
                RawEntryResult::KnownType(e) => entries.push(e),
                RawEntryResult::UnknownType(tag, v) => {
                    println!("Unknown type for tag {:?}: {:?}", tag, v);
                }
                RawEntryResult::InvalidCount(tag, c) => {
                    println!("Invalid count for tag {:?}: {:?}", tag, c);
                }
            }
        }
        let next_ifd_offset = match self {
            TIFFVariant::Classic => decode_u32(
                ifd_data[fields_count * self.ifd_entry_size()..]
                    .try_into()
                    .unwrap(),
            ) as u64,
            TIFFVariant::BigTiff => decode_u64(
                ifd_data[fields_count * self.ifd_entry_size()..]
                    .try_into()
                    .unwrap(),
            ),
        };
        Ok((ImageFileDirectory { entries }, next_ifd_offset as u64))
    }

    async fn decode_ifd_entry_metadata(&self, buf: &[u8]) -> Result<RawEntryResult, Error> {
        // Check buf len is correct
        let expected_len = match self {
            TIFFVariant::Classic => 12,
            TIFFVariant::BigTiff => 20,
        };
        if buf.len() != expected_len {
            return Err(Error::InvalidData(format!(
                "ifd entry has len={}, expected={}",
                buf.len(),
                expected_len
            )));
        }

        let tag = decode_tag(decode_u16([buf[0], buf[1]]));
        let field_type = decode_u16([buf[2], buf[3]]);
        let field_type = match field_type {
            0 => return Ok(RawEntryResult::UnknownType(tag, 0)),
            v @ 19.. => return Ok(RawEntryResult::UnknownType(tag, v)),
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
            v @ 13..=15 => return Ok(RawEntryResult::UnknownType(tag, v)),
            16 => IFDType::Unsigned64,
            17 => IFDType::Signed64,
            18 => IFDType::Unsigned64, // This is an unsigned offset type, but we just consider it equivalent to u64
        };
        let (count, offset_or_value) = match self {
            TIFFVariant::Classic => {
                let count = decode_u32_from_slice(&buf[4..8]) as u64;
                let offset_or_value: OffsetOrInlineValue = {
                    if type_size(field_type) * count as usize <= 4 {
                        OffsetOrInlineValue::FourBytesInlineValue([
                            buf[8], buf[9], buf[10], buf[11],
                        ])
                    } else {
                        OffsetOrInlineValue::Offset(decode_u32_from_slice(&buf[8..12]) as u64)
                    }
                };
                (count, offset_or_value)
            }
            TIFFVariant::BigTiff => {
                let count = decode_u64_from_slice(&buf[4..12]);
                let offset_or_value: OffsetOrInlineValue = {
                    if type_size(field_type) * count as usize <= 8 {
                        let mut data = [0u8; 8];
                        data.copy_from_slice(&buf[12..20]);
                        OffsetOrInlineValue::EightBytesInlineValue(data)
                    } else {
                        OffsetOrInlineValue::Offset(decode_u64_from_slice(&buf[12..20]))
                    }
                };
                (count, offset_or_value)
            }
        };
        if count == 0 {
            return Ok(RawEntryResult::InvalidCount(tag, count));
        }
        Ok(RawEntryResult::KnownType(IFDEntryMetadata {
            tag,
            field_type,
            count,
            offset_or_value,
        }))
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct TIFFReader {
    pub ifds: Vec<ImageFileDirectory>,
    #[cfg_attr(feature = "json", serde(skip_serializing))]
    pub source: Source,
}

impl TIFFReader {
    pub async fn open_from_source_spec(source_spec: &str) -> Result<TIFFReader, Error> {
        let source = Source::new_from_source_spec(source_spec).await?;
        let reader = Self::open_from_source(source).await?;
        Ok(reader)
    }
    pub async fn open_from_source(mut source: Source) -> Result<TIFFReader, Error> {
        // Byte order & magic number check
        {
            let mut buf = [0u8; 2];
            source.read_exact(0, &mut buf[..]).await?;
            if buf[0] == 0x49 && buf[1] == 0x49 {
                // Ok (little endian)
            } else if buf[0] == 0x4D && buf[1] == 0x4D {
                return Err(Error::InvalidData(
                    "Big endian files not supported".to_string(),
                ));
            } else {
                return Err(Error::InvalidData(format!("Invalid byte_order {:?}", buf)));
            }
        }
        let variant: TIFFVariant = {
            let magic_number = read_u16(&mut source, 2).await?;
            match magic_number {
                42 => Ok(TIFFVariant::Classic),
                43 => Ok(TIFFVariant::BigTiff),
                _ => Err(Error::InvalidData(format!(
                    "Invalid magic_number {:?}",
                    magic_number
                ))),
            }
        }?;

        let initial_ifd_offset: u64 = variant.read_initial_ifd_offset(&mut source).await?;

        // Read ifds
        let ifds: Vec<ImageFileDirectory> = {
            let mut ifds = vec![];
            let mut ifd_offset = initial_ifd_offset;
            // TODO: Infinite loop detection ?
            while ifd_offset > 0 {
                let (ifd, next_ifd_offset) = variant
                    .read_image_file_directory(&mut source, ifd_offset)
                    .await?;
                ifd_offset = next_ifd_offset;
                ifds.push(ifd);
            }
            ifds
        };

        Ok(TIFFReader { ifds, source })
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
}
