"""
Utility to compare to XYZ tiles extracted by rio-tiler
"""
import argparse
from rio_tiler.io import Reader
import pylab as pl

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="compare_to_rio_tiler"
    )
    parser.add_argument("filename")
    parser.add_argument("tile_x", type=int)
    parser.add_argument("tile_y", type=int)
    parser.add_argument("tile_z", type=int)

    args = parser.parse_args()

    with Reader(args.filename) as image:
        tile = image.tile(args.tile_x, args.tile_y, args.tile_z)
        img = tile.data.transpose(1, 2, 0)
        # tile also has a .mask that we should compare against
    pl.imshow(img)
    pl.show()
