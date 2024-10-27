"""
Utility to compare to XYZ tiles extracted by rio-tiler
"""
import argparse
from rio_tiler.io import Reader
import numpy as np
import pylab as pl

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="compare_to_rio_tiler"
    )
    parser.add_argument("filename")
    parser.add_argument("z", type=int)
    parser.add_argument("x", type=int)
    parser.add_argument("y", type=int)
    parser.add_argument("--bands", type=str, default="0,1,2", required=False)
    parser.add_argument("--vmin", type=float, default=0, required=False)
    parser.add_argument("--vmax", type=float, default=255, required=False)

    args = parser.parse_args()

    bands = [int(v) for v in args.bands.split(",")]
    vmin = args.vmin
    vmax = args.vmax

    with Reader(args.filename) as image:
        tile = image.tile(args.x, args.y, args.z)
        img = tile.data.transpose(1, 2, 0)
        # select bands and normalize as per commandline arguments
        img = img[:, :, bands]
        img = (img - vmin) / (vmax - vmin)
        img = (img * 255).astype(np.uint8)
        # tile also has a .mask that we should compare against
    pl.imshow(img)
    pl.show()
