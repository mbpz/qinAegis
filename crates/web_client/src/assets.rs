/// Embed HTML, CSS, and JS assets as const strings.
/// Used by main.rs custom protocol "app://" to serve embedded assets.

pub fn getAsset(path: &str) -> (&'static [u8], &'static str) {
    match path {
        "index.html" => (include_bytes!("../index.html").as_slice(), "text/html; charset=utf-8"),
        "style.css" => (include_bytes!("../style.css").as_slice(), "text/css; charset=utf-8"),
        "app.js" => (include_bytes!("../app.js").as_slice(), "application/javascript; charset=utf-8"),
        _ => (b"404 Not Found", "text/plain"),
    }
}

pub fn getAssetStr(path: &str) -> (&'static str, &'static str) {
    match path {
        "index.html" => (include_str!("../index.html"), "text/html; charset=utf-8"),
        "style.css" => (include_str!("../style.css"), "text/css; charset=utf-8"),
        "app.js" => (include_str!("../app.js"), "application/javascript; charset=utf-8"),
        _ => ("404 Not Found", "text/plain"),
    }
}