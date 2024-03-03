"""
Utility to compare to XYZ tiles extracted by rio-tiler
"""
import argparse
from rio_tiler.io import Reader
import pylab as pl
import numpy as np

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="compare_to_rio_tiler"
    )
    parser.add_argument("filename")
    parser.add_argument("z", type=int)
    parser.add_argument("x", type=int)
    parser.add_argument("y", type=int)

    args = parser.parse_args()

    with Reader(args.filename) as image:
        tile = image.tile(args.x, args.y, args.z)
        img = tile.data.transpose(1, 2, 0)
        # tile also has a .mask that we should compare against
    np.save('rio_tile.npy', img)
