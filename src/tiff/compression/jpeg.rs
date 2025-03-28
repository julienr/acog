// In a TIFF, jpeg images are stored in two parts:
// - An "abbreviated table specification", which contains the huffmann tables and is common
//   to all the tiles in the image. This is stored once in a "JpegTables" TIFF tag
// - Each tile is then stored as an "abbreviated image", which means it contains just the compressed
//   image data, not the tables
//
// More details here: https://download.osgeo.org/libtiff/old/TTN2.draft.txt
//
// This is done to save space and improve performance (tables only need to be loaded once). But
// decoding those two separately require a decoder that support that.
//
// GDAL uses turbojpeg and it's possible to decode first the tables and then images using the C
// API, but it's quite convoluted. turbojpeg v3 also supports tj3DecompressHeader which can take
// just the JPEG tables. So using turbojpeg v3 through the CFFI would be an option
//
// zune-jpeg doesn't seem to support that yet - it needs a whole jpeg image in one go. So we
// rebuild that full image by concatenating the jpeg tables and the data stream. Doing so is not
// optimal performance-wise though if we would read many tiles from the same COG at once
//
// ==== References
// A GDAL COG seem to define JPEG tags:
// - 347 = JPEG tables
// - 530 = TIFFTAG_YCBCRSUBSAMPLING
// - 532 = TIFFTAG_REFERENCEBLACKWHITE
// https://github.com/OSGeo/gdal/blob/7d3e653b5ed80f281d8664ee4bb217b24d9980bf/frmts/gtiff/libtiff/tiff.h#L345C9-L345C27
//
// GDAL uses the libjpeg API to decode:
// https://github.com/OSGeo/gdal/blob/7d3e653b5ed80f281d8664ee4bb217b24d9980bf/frmts/gtiff/libtiff/tif_jpeg.c#L1092

use crate::Error;
use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

#[derive(Clone)]
pub struct Decompressor {
    jpeg_tables: Vec<u8>,
}

// https://www.disktuna.com/list-of-jpeg-markers/
const START_OF_IMAGE: [u8; 2] = [0xff, 0xd8];
const END_OF_IMAGE: [u8; 2] = [0xff, 0xd9];

impl Decompressor {
    pub fn new(jpeg_tables: &[u8]) -> Result<Decompressor, Error> {
        Ok(Decompressor {
            jpeg_tables: jpeg_tables.into(),
        })
    }

    pub fn decompress(
        &self,
        data: Vec<u8>,
        _width: usize,
        _height: usize,
    ) -> Result<Vec<u8>, Error> {
        // Since zune-jpeg doesn't support decoding first the huffman tables only and then the images
        // from two stream, we concatenate both in one stream. This is suboptimal performance-wise,
        // but it works.
        //
        // Both the jpeg_tables ("abbreviated table specification") and the data ("abbreviated image")
        // will start with the START_OF_IMAGE (SOI) jpeg marker and end with the END_OF_IMAGE (EOI) one.
        if self.jpeg_tables[self.jpeg_tables.len() - 2] != END_OF_IMAGE[0]
            || self.jpeg_tables[self.jpeg_tables.len() - 1] != END_OF_IMAGE[1]
        {
            return Err(Error::DecompressionError(format!(
                "Expected END_OF_IMAGE marker ({:x?}, found {:x?}",
                END_OF_IMAGE,
                &self.jpeg_tables[self.jpeg_tables.len() - 2..],
            )));
        }

        if data[0] != START_OF_IMAGE[0] || data[1] != START_OF_IMAGE[1] {
            return Err(Error::DecompressionError(format!(
                "Expected START_OF_IMAGE marker ({:x?}, found {:x?}",
                START_OF_IMAGE,
                &data[0..2],
            )));
        }

        // Concatenate both, removing last 2 bytes (EOI) of the tables stream and first 2 bytes
        // (SOI) of the data stream so that it looks like a single stream
        let jpeg_image: Vec<u8> = [
            &self.jpeg_tables.as_slice()[..self.jpeg_tables.len() - 2],
            &data[2..],
        ]
        .concat();
        let mut decoder = JpegDecoder::new(ZCursor::new(jpeg_image));
        let pixels = decoder.decode()?;
        Ok(pixels)
    }
}
