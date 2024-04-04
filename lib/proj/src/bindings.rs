// We use include here because in the future we should have build.rs figure out the proj version to link against and then
// include the correct pre-generated bindings. See the georust gdal bindings for example
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!("prebuilt-bindings/proj_8_2.rs");
