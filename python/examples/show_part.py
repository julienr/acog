import acog
import asyncio
import argparse
import numpy as np
import pylab as pl


async def main(filename, overview_index, x_from, y_from, x_to, y_to, band):
    image_tile = await acog.read_subimage(
        filename, overview_index, x_from, y_from, x_to, y_to
    )
    img = np.frombuffer(
        image_tile.data_buffer(), dtype=np.dtype(image_tile.dtype())
    ).reshape(image_tile.height(), image_tile.width(), image_tile.nbands())
    pl.title(f"{band=}")
    pl.imshow(img[:, :, band], cmap="viridis")
    pl.colorbar()
    pl.show()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(prog="acog_example")
    parser.add_argument("filename")
    parser.add_argument("overview_index", type=int)
    parser.add_argument("x_from", type=int)
    parser.add_argument("y_from", type=int)
    parser.add_argument("x_to", type=int)
    parser.add_argument("y_to", type=int)
    parser.add_argument("band", type=int)
    args = parser.parse_args()
    asyncio.run(
        main(
            args.filename,
            args.overview_index,
            args.x_from,
            args.y_from,
            args.x_to,
            args.y_to,
            args.band,
        )
    )
