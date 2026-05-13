/// Embed React build assets from src/assets/ directory.
/// Used by main.rs custom protocol "app://" to serve embedded assets.

pub fn getAsset(path: &str) -> (&'static [u8], &'static str) {
    match path {
        // React build output
        "index.html" => (include_bytes!("../src/assets/index.html").as_slice(), "text/html; charset=utf-8"),
        _ if path.starts_with("assets/") => {
            let full_path = format!("../src/assets/{}", path);
            match include_bytes!(full_path.as_str()) {
                Ok(bytes) => (bytes, mime_type(path)),
                Err(_) => (b"404 Not Found", "text/plain"),
            }
        }
        _ => (b"404 Not Found", "text/plain"),
    }
}

pub fn getAssetStr(path: &str) -> (&'static str, &'static str) {
    match path {
        "index.html" => (include_str!("../src/assets/index.html"), "text/html; charset=utf-8"),
        _ if path.starts_with("assets/") => {
            let full_path = format!("../src/assets/{}", path);
            match include_str!(full_path.as_str()) {
                Ok(s) => (s, mime_type(path)),
                Err(_) => ("404 Not Found", "text/plain"),
            }
        }
        _ => ("404 Not Found", "text/plain"),
    }
}

fn mime_type(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}
