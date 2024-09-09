import acog
import pytest
import numpy as np
from PIL import Image


@pytest.mark.asyncio
async def test_read_tile_local_file_partial_tile():
    image_tile = await acog.read_tile(
        "../../example_data/example_1_cog_3857_nocompress.tif",
        20, 549689, 365591
    )

    img = np.frombuffer(image_tile.data_buffer(), dtype=np.uint8).reshape(
        image_tile.height(), image_tile.width(), image_tile.nbands()
    )
    assert img.dtype == np.uint8
    assert img.shape[0] == 256
    assert img.shape[1] == 256
    assert img.shape[2] == 3

    expected = np.array(
        Image.open(
            '../../example_data/tests_expected/example_1_cog_3857_nocompress__20_549689_365591.ppm'  # noqa(E501)
        )
    )
    assert np.all(img == expected)
