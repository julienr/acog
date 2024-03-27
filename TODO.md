### TODOs

- [ ] Implement chunked reading
  - With HTTP ranges in particular, need to gracefully handle being out of range due to chunk
- [ ] Implement global LRU cache like GDAL (16GB shared across all tasks)
- [ ] Add test for `find_best_overview` or cover this in extract_tile test above.
      Basically testing we read from correct overview
- [ ] [BigTiff support](https://www.awaresystems.be/imaging/tiff/bigtiff.html)