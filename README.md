# acog - An async rust library to read Cloud-Optimized GeoTiffs

This is currently very early stage software. My main goals are:

- Learning more about COGs
- GDAL being sync, see if having an async library can improve performance of typical "tiler" servers (like [rio-tiler](https://cogeotiff.github.io/rio-tiler/))

# References

- TIFF standard: http://download.osgeo.org/geotiff/spec/tiff6.pdf
- BigTIFF: https://www.awaresystems.be/imaging/tiff/bigtiff.html
- GeoTIFF standard: https://docs.ogc.org/is/19-008r4/19-008r4.html
- COG standard: https://docs.ogc.org/is/21-026/21-026.html
- DEFLATE/JPEG technical notes from Adobe: https://www.awaresystems.be/imaging/tiff/specification/TIFFphotoshop.pdf
- TIFF tags directory: https://www.awaresystems.be/imaging/tiff.html

## GDAL warping

- https://github.com/OSGeo/gdal/blob/b63f9ad1881853f000b054c7dd787090da1fb9dc/alg/gdalwarper.cpp#L1215

## GDAL vsi

- https://gdal.org/user/virtual_file_systems.html#vsicurl-http-https-ftp-files-random-access

In particular the section about vsicurl has notes on caching and chunk/request size

## Useful GDAL commands

`gdalwarp -of COG -t_srs "EPSG:3857" -co "COMPRESS=NONE" marina_cog_nocompress.tif marina_cog_nocompress_3857.tif`

## Notes

Testing extract_tile on a local file:

```
cargo run --bin extract_tile -- example_data/local/marina_cog_nocompress_3857.tif 18 215827 137565
cargo run --bin extract_tile -- example_data/example_1_cog_3857_nocompress_bigtiff.tif 20 549687 365589
```

Testing extract_tile on a minio hosted file:

```
cargo run --bin extract_tile -- /vsis3/public/local/marina_cog_nocompress_3857.tif 18 215827 137565 && eog img.ppm
```

Testing extract_tile on a GCS hosted file:

```
export GOOGLE_SERVICE_ACCOUNT_CONTENT=$(cat service_account.json)
cargo run --bin extract_tile -- /vsigs/acog-test/marina/marina_split_1_cog.tif 18 215827 137565 && eog img.ppm
```

Testing extract_tile through python bindings:

```
cd python/acog
maturin develop && python examples/extract_tile.py ../../example_data/example_1_cog_nocompress.tif 20 549687 365589
```

GDAL info on a COG on minio

```
export AWS_S3_ENDPOINT=localhost:9000
export AWS_HTTPS=NO
export AWS_VIRTUAL_HOSTING=FALSE
export AWS_NO_SIGN_REQUEST=YES
CPL_DEBUG=ON gdalinfo /vsis3/public/local/marina_cog_nocompress_3857.tif
```

Testing range requests headers with curl:

`curl -v -r 558379749-558379761 http://localhost:9000/public/local/marina_cog_nocompress_3857.tif`

### Test in QGIS


`cargo run -p example-tileserver`

And then connect as a XYZ source in QGIS with the url:

`http://localhost:3000/tile/example_data/local/marina_cog_nocompress_3857.tif/{z}/{x}/{y}`

You can also debug tiles with e.g.:

http://localhost:3000/example_data%2Flocal%2Fmarina_cog_nocompress_3857.tif/tile/18/215827/137565

## Running automated tests

### Testing GCS integration

To test GCS integration/authentication, you need a service account key and include
its content in the `GOOGLE_SERVICE_ACCOUNT_CONTENT` env variable:

```
export GOOGLE_SERVICE_ACCOUNT_CONTENT=$(cat service_account.json)
cargo test -- --ignored
```

The `--ignored` is required because those tests are ignored by default.