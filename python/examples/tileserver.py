"""
Example of implementing a tileserver in python
"""

import acog
import json
from aiohttp import web
from PIL import Image
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
        }
    </style>
</head>
<body>
    <div id="map"></div>
    <div class="overlay">
        <input
            type="text"
            id="filename"
            size="100"
            value="/vsis3/public/local/marina_cog_nocompress_3857.tif">
        </input>
        <button onclick="onView()">View</button>
    </div>
    <script>
        const map = L.map('map').setView([0, 0], 4);
        const osm = L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
            maxZoom: 19,
            attribution: '&copy; <a href="http://www.openstreetmap.org/copyright">OpenStreetMap</a>'
        }).addTo(map);

        let tileLayer = null;

        async function onView () {
            const input = document.getElementById('filename');
            const filename = input.value;

            const resp = await fetch("/bounds/" + filename);
            if (!resp.ok) {
                throw new Error(`Response status ${resp.status}`);
            }
            const bounds = await resp.json();
            map.fitBounds(L.latLngBounds(
                L.latLng(bounds.lat_min, bounds.lng_min),
                L.latLng(bounds.lat_max, bounds.lng_max)
            ));

            const url = "/tile/" + filename + "/{z}/{x}/{y}";
            if (tileLayer !== null) {
                map.removeLayer(tileLayer);
            }
            tileLayer = L.tileLayer(url, { maxZoom: 24 }).addTo(map);
        }
        // Setup initial view
        onView();
    </script>
</body>
"""


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
    assert image_tile.nbands() == 3
    img = Image.frombuffer(
        "RGB", (image_tile.width(), image_tile.height()), image_tile.data_buffer()
    )
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
