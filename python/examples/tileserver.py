"""
Example of implementing a tileserver in python
"""

import acog
import json
from aiohttp import web
from PIL import Image
import numpy as np
from io import BytesIO

INDEX_HTML = """
<head>
    <link
        rel="stylesheet"
        href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css"
        integrity="sha256-p4NxAoJBhIIN+hmNHrzRCf9tD/miZyoHS5obTRR9BMY="
        crossorigin=""/>
    <script
        src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"
        integrity="sha256-20nQCchB9co0qIjJZRGuk2/Z9VM+kNiyxNV1lvTlZBo="
        crossorigin=""></script>
    <style>
        html, body {
            height: 100%;
            margin: 0;
        }
        #map {
            position: relative;
            height: 100%;
        }
        .overlay {
            background-color: #fff;
            padding: 12px;
            z-index: 1001;
            position: absolute;
            bottom: 20px;
            left: 20px;
            display: flex;
            flex-direction: column;
        }
    </style>
</head>
<body>
    <div id="map"></div>
    <div class="overlay">
        <div class="row">
            file:
            <input
                type="text"
                id="filename"
                size="60"
                value="/vsis3/public/local/marina_cog_nocompress_3857.tif">
            </input>
            or example
            <select
                id="example"
                onchange="exampleSelected()">
                <!-- Options are encoded as "<url>|<params>" -->
                <option value="/vsis3/public/local/marina_cog_nocompress_3857.tif|">marina</option>
                <option value="/vsis3/public/local/marina_cog_nocompress_3857.tif|bands=1,1,1&vmax=200">marina gray</option>
                <option value="/vsis3/public/example_1_cog_jpeg.tif|">example 1</option>
                <option value="/vsis3/public/s2_corsica_1.tiff|bands=1,2,3&vmax=0.5">s2 corsica 1-3</option>
            </select>
        </div>
        <div class="row">
            params:
            <input
                type="text"
                id="params"
                size="20"
                value="">
            </input> (e.g. ?bands=0,1,2 or ?bands=1 ?vmin=0 ?vmax=255)
        </div>
        <div class="row">
            <button onclick="onView()">View</button>
        </div>
    </div>
    <script>
        const map = L.map('map').setView([0, 0], 4);
        const osm = L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
            maxZoom: 19,
            attribution: '&copy; <a href="http://www.openstreetmap.org/copyright">OpenStreetMap</a>'
        }).addTo(map);

        let tileLayer = null;

        function exampleSelected () {
            const example = document.getElementById('example').value;
            const [url, params] = example.split('|');
            console.log('example', url, params);
            document.getElementById('filename').value = url;
            document.getElementById('params').value = params;
            onView();
        }

        async function onView () {
            const input = document.getElementById('filename');
            const filename = input.value;
            const params = document.getElementById('params').value;

            const resp = await fetch("/bounds/" + filename);
            if (!resp.ok) {
                throw new Error(`Response status ${resp.status}`);
            }
            const bounds = await resp.json();
            map.fitBounds(L.latLngBounds(
                L.latLng(bounds.lat_min, bounds.lng_min),
                L.latLng(bounds.lat_max, bounds.lng_max)
            ));

            const url = "/tile/" + filename + "/{z}/{x}/{y}" + "?" + params;
            console.log(url);
            if (tileLayer !== null) {
                map.removeLayer(tileLayer);
            }
            tileLayer = L.tileLayer(url, { maxZoom: 24 }).addTo(map);
        }
        // Setup initial view
        onView();
    </script>
</body>
"""  # noqa(E501)


async def index(request):
    return web.Response(body=INDEX_HTML, content_type="text/html")


async def bounds(request):
    filename = request.match_info.get("filename")
    bounds = await acog.get_bounds(filename)
    bounds = {
        "lng_min": bounds.xmin(),
        "lat_min": bounds.ymin(),
        "lng_max": bounds.xmax(),
        "lat_max": bounds.ymax(),
    }
    return web.Response(body=json.dumps(bounds), content_type="application/json")


async def tile(request):
    z = int(request.match_info.get("z"))
    x = int(request.match_info.get("x"))
    y = int(request.match_info.get("y"))
    filename = request.match_info.get("filename")
    image_tile = await acog.read_tile(filename, z, x, y)
    arr = np.frombuffer(
        image_tile.data_buffer(), dtype=np.dtype(image_tile.dtype())
    ).reshape(image_tile.height(), image_tile.width(), image_tile.nbands())
    bands = [int(v) for v in request.query.get("bands", "0,1,2").split(",")]
    vmin = float(request.query.get("vmin", "0"))
    vmax = float(request.query.get("vmax", "255"))
    if len(bands) == 1:
        arr = np.repeat(arr[:, :, bands[0]], 3, axis=2)
    else:
        assert len(bands) == 3
        arr = arr[:, :, bands]
    print(f"{vmin=}, {vmax=}, {bands=}")

    arr = np.clip((arr.astype(np.float64) - vmin) / (vmax - vmin), min=0, max=1)
    img = Image.fromarray(np.uint8(arr * 255))
    # img = Image.fromarray(np.uint8(arr))
    buffer = BytesIO()
    img.save(buffer, format="PNG")
    return web.Response(body=buffer.getvalue(), content_type="image/png")


app = web.Application()
app.add_routes(
    [
        web.get("/", index),
        web.get(r"/tile/{filename:.+}/{z}/{x}/{y}", tile),
        web.get(r"/bounds/{filename:.+}", bounds),
    ]
)


if __name__ == "__main__":
    web.run_app(app)
