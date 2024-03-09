# TODO

- [BigTiff support](https://www.awaresystems.be/imaging/tiff/bigtiff.html)

# References

- TIFF standard: http://download.osgeo.org/geotiff/spec/tiff6.pdf
- GeoTIFF standard: https://docs.ogc.org/is/19-008r4/19-008r4.html
- COG standard: https://docs.ogc.org/is/21-026/21-026.html

## GDAL warping

- https://github.com/OSGeo/gdal/blob/b63f9ad1881853f000b054c7dd787090da1fb9dc/alg/gdalwarper.cpp#L1215

## Useful GDAL commands

`gdalwarp -of COG -t_srs "EPSG:3857" -co "COMPRESS=NONE" marina_cog_nocompress.tif marina_cog_nocompress_3857.tif`

## Notes

`cargo run --bin extract_tile -- example_data/local/marina_cog_nocompress_3857.tif 18 215827 137565`

### Test in QGIS


`cargo run -p example-tileserver`

And then connect as a XYZ source in QGIS with the url:

`http://localhost:3000/tile/example_data/local/marina_cog_nocompress_3857.tif/{z}/{x}/{y}`

You can also debug tiles with e.g.:

http://localhost:3000/example_data%2Flocal%2Fmarina_cog_nocompress_3857.tif/tile/18/215827/137565