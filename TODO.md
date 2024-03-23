### TODOs

- [ ] Handle incomplete tiles in tiler. For example
      `cargo run --bin extract_tile -- /vsis3/public/example_1_cog_3857_nocompress.tif 20 549689 365591 && python3 utils/npyshow.py img.npy`
- [ ] Add test for `find_best_overview` or cover this in extract_tile test above.
      Basically testing we read from correct overview
- [ ] [BigTiff support](https://www.awaresystems.be/imaging/tiff/bigtiff.html)