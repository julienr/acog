import acog
import asyncio
import argparse
import numpy as np
import pylab as pl


async def main(filename, z, x, y):
  tile_data_buf = await acog.read_tile(filename, z, x, y)
  # TODO: Hardcoded shape => return a struct from python containing some shape info
  arr = np.frombuffer(tile_data_buf, dtype=np.uint8).reshape(256, 256, 3)
  pl.imshow(arr)
  pl.show()


if __name__ == '__main__':
  parser = argparse.ArgumentParser(prog="acog_example")
  parser.add_argument('filename')
  parser.add_argument('z', type=int)
  parser.add_argument('x', type=int)
  parser.add_argument('y', type=int)
  args = parser.parse_args()
  asyncio.run(main(args.filename, args.z, args.x, args.y))
