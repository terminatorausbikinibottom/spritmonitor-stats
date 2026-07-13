//! Static assets, embedded into the binary so the deployment is a single
//! file and works in an offline LXC.

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

const ASSETS: &[(&str, &str, &[u8])] = &[
    (
        "htmx.min.js",
        "application/javascript",
        include_bytes!("../../assets/htmx.min.js"),
    ),
    (
        "uplot.iife.min.js",
        "application/javascript",
        include_bytes!("../../assets/uplot.iife.min.js"),
    ),
    (
        "uplot.min.css",
        "text/css",
        include_bytes!("../../assets/uplot.min.css"),
    ),
    ("app.css", "text/css", include_bytes!("../../assets/app.css")),
    (
        "app.js",
        "application/javascript",
        include_bytes!("../../assets/app.js"),
    ),
];

pub async fn serve(Path(file): Path<String>) -> Response {
    match ASSETS.iter().find(|(name, _, _)| *name == file) {
        Some((_, content_type, bytes)) => (
            [
                (header::CONTENT_TYPE, *content_type),
                (header::CACHE_CONTROL, "public, max-age=3600"),
            ],
            *bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
