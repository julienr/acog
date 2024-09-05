use std::future::Future;

use std::num::ParseIntError;

use acog::tiler::{extract_tile, TMSTileCoords};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body;
use hyper::http;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode};
use std::pin::Pin;

use hyper_util::rt::tokio::{TokioIo, TokioTimer};

const INDEX: &str = include_str!("index.html");

enum Error {
    Http(http::Error),
    Acog(acog::Error),
    Other(String),
}

impl From<http::Error> for Error {
    fn from(value: hyper::http::Error) -> Self {
        Error::Http(value)
    }
}

impl From<acog::Error> for Error {
    fn from(value: acog::Error) -> Self {
        Error::Acog(value)
    }
}

impl From<ParseIntError> for Error {
    fn from(value: ParseIntError) -> Self {
        Error::Other(format!("ParseIntError: {:?}", value))
    }
}

impl From<turbojpeg::Error> for Error {
    fn from(value: turbojpeg::Error) -> Self {
        Error::Other(format!("turbojpeg error: {:?}", value))
    }
}

type HandlerResponse = Response<Full<Bytes>>;

fn check_filename(filename: &str) -> Result<(), Error> {
    // TODO(security): This feels basic
    if filename.contains("..") {
        Err(Error::Other("Invalid filename".to_string()))
    } else {
        Ok(())
    }
}

async fn four_oh_four(method: &hyper::Method, path: &str) -> Result<HandlerResponse, Error> {
    println!("Not found: method={}, path={}", method, path);
    let builder = Response::builder().status(StatusCode::NOT_FOUND);
    Ok(builder.body("Not found".to_string().into_bytes().into())?)
}

// basic handler that responds with a static string
async fn index() -> Result<HandlerResponse, Error> {
    let builder = Response::builder().status(StatusCode::OK);
    Ok(builder.body(INDEX.to_string().into_bytes().into())?)
}

async fn get_bounds(filename: &str) -> Result<HandlerResponse, Error> {
    check_filename(filename)?;
    println!("get_bounds {}", filename);
    let cog = acog::COG::open(filename).await?;
    let bbox = cog.lnglat_bounds()?;
    let bbox_json_str = format!(
        "{{\n\
            \"lng_min\": {},\n\
            \"lng_max\": {},\n\
            \"lat_min\": {},\n\
            \"lat_max\": {}\n\
         }}",
        bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax
    );
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(bbox_json_str.into_bytes().into())?)
}

async fn get_tile(filename: &str, z: u32, x: u64, y: u64) -> Result<HandlerResponse, Error> {
    check_filename(filename)?;
    println!("get_tile {} {} {} {}", filename, z, x, y);
    let mut cog = acog::COG::open(filename).await?;
    let tile_data = extract_tile(&mut cog, TMSTileCoords::from_zxy(z, x, y)).await?;
    // Encode to jpeg using turbojpeg and send back data
    let img = turbojpeg::Image::<&[u8]> {
        pixels: &tile_data.img.data,
        width: tile_data.img.width,
        height: tile_data.img.height,
        pitch: tile_data.img.width * 3,
        format: turbojpeg::PixelFormat::RGB,
    };
    let jpeg_buf = turbojpeg::compress(img, 95, turbojpeg::Subsamp::Sub2x2)?;
    let jpeg_data: Vec<u8> = jpeg_buf.as_ref().to_vec();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/jpeg")
        .body(Full::from(jpeg_data))?)
}

#[derive(Clone)]
struct MainService {}

fn error_response<E>(
    method: &http::Method,
    path: &str,
    e: E,
) -> Result<HandlerResponse, http::Error>
where
    E: std::fmt::Debug,
{
    println!(
        "Error handling request method={}, path={}: {:?}",
        method, path, e
    );
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body("Internal error".to_string().into_bytes().into())
}

async fn router_inner(method: &Method, path: &str) -> Result<HandlerResponse, Error> {
    // The path we get should always start with a slash
    let path_parts: Vec<&str> = path[1..].split('/').collect();
    // println!("path '{}', path_parts {:?}", path, path_parts);
    let part_match =
        |index: usize, val: &str| -> bool { index < path_parts.len() && path_parts[index] == val };

    // The below routing is quite verbose and manual and you can make it a bit more magic (see [1]). On
    // the other hand, it nicely handles passing a filename with slashes that may not always
    // be urlencoded (e.g. when using QGIS)
    //
    // [1] https://github.com/gypsydave5/todo-mvp/blob/master/todos/rust/src/main.rs
    if method == Method::GET {
        if path_parts.len() == 1 && (part_match(0, "") || part_match(0, "index.html")) {
            // "/" or "/index.html"
            index().await
        } else if part_match(0, "tile") && path_parts.len() >= 5 {
            // "tile/raster.tif/{z}/{x}/{y}"
            // "tile/example_data/local/raster.tif/{z}/{x}/{y}"
            // "tile//vsis3/example_data/local/raster.tif/{z}/{x}/{y}"
            let n = path_parts.len();
            let z = path_parts[n - 3].parse::<u32>()?;
            let x = path_parts[n - 2].parse::<u64>()?;
            let y = path_parts[n - 1].parse::<u64>()?;
            let filename = path_parts[1..n - 3].join("/");
            get_tile(&filename, z, x, y).await
        } else if part_match(0, "bounds") && path_parts.len() >= 2 {
            // "bounds/raster.tif"
            // "bounds/example_data/local/raster.tif"
            // "bounds//vsis3/example_data/local/raster.tif"
            let filename = path_parts[1..].join("/");
            get_bounds(&filename).await
        } else {
            four_oh_four(method, path).await
        }
    } else {
        four_oh_four(method, path).await
    }
}

async fn router(req: Request<body::Incoming>) -> Result<HandlerResponse, http::Error> {
    let res = router_inner(req.method(), req.uri().path()).await;
    match res {
        Ok(v) => Ok(v),
        Err(Error::Http(e)) => Err(e),
        Err(Error::Acog(e)) => error_response(req.method(), req.uri().path(), e),
        Err(Error::Other(e)) => error_response(req.method(), req.uri().path(), e),
    }
}

impl Service<Request<body::Incoming>> for MainService {
    type Response = Response<Full<Bytes>>;
    type Error = http::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<body::Incoming>) -> Self::Future {
        Box::pin(router(req))
    }
}

#[tokio::main]
async fn main() {
    // run our app with hyper
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    let service = MainService {};
    loop {
        let (tcp, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(tcp);
        let svc_clone = service.clone();
        tokio::task::spawn(async move {
            let res = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, svc_clone);

            if let Err(err) = res.await {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
