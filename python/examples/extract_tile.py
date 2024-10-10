import acog
import asyncio
import argparse
import numpy as np
import pylab as pl


async def main(filename, z, x, y):
    bounds = await acog.get_bounds(filename)
    print(f"xmin={bounds.xmin()}, ymin={bounds.ymin()}")
    image_tile = await acog.read_tile(filename, z, x, y)
    img = np.frombuffer(image_tile.data_buffer(), dtype=np.uint8).reshape(
        image_tile.height(), image_tile.width(), image_tile.nbands()
    )
    pl.imshow(img)
    pl.show()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(prog="acog_example")
    parser.add_argument("filename")
    parser.add_argument("z", type=int)
    parser.add_argument("x", type=int)
    parser.add_argument("y", type=int)
    args = parser.parse_args()
    asyncio.run(main(args.filename, args.z, args.x, args.y))
