import os
import acog
import pytest
import numpy as np
from PIL import Image

BASE_DIR = os.path.dirname(__file__)


@pytest.mark.asyncio
async def test_read_tile_local_file_partial_tile():
    input_fname = os.path.join(
        BASE_DIR, "../../example_data/example_1_cog_3857_nocompress.tif"
    )
    expected_fname = os.path.join(
        BASE_DIR,
        "../../example_data/tests_expected/example_1_cog_3857_nocompress__20_549689_365591.ppm",
    )
    image_tile = await acog.read_tile(
        input_fname,
        20,
        549689,
        365591,
    )

    img = np.frombuffer(image_tile.data_buffer(), dtype=np.uint8).reshape(
        image_tile.height(), image_tile.width(), image_tile.nbands()
    )
    assert img.dtype == np.uint8
    assert img.shape[0] == 256
    assert img.shape[1] == 256
    assert img.shape[2] == 3

    expected = np.array(Image.open(expected_fname))
    assert np.all(img == expected)
