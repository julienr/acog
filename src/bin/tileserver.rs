use axum::{Router, routing::get};

#[tokio::main]
pub async fn main () {
    let app = Router::new()
        .route("/", get(root))
        .route("/tile", get(get_tile));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root () -> &'static str {
    "Demo tileserver"
}

async fn get_tile () -> String {
    "get_tile".to_string()
}