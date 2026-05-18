/// Embed HTML, CSS, and JS assets as const strings.
/// Used by main.rs custom protocol "app://" to serve embedded assets.

pub fn getAsset(path: &str) -> (&'static [u8], &'static str) {
    match path {
        "index.html" => (include_bytes!("../src/assets/index.html").as_slice(), "text/html; charset=utf-8"),
        "assets/index.css" | "assets/index-BQHUhsXK.css" => (include_bytes!("../src/assets/assets/index-BQHUhsXK.css").as_slice(), "text/css; charset=utf-8"),
        "assets/index.js" | "assets/index-C6mVjCLc.js" => (include_bytes!("../src/assets/assets/index-C6mVjCLc.js").as_slice(), "application/javascript; charset=utf-8"),
        _ => (b"404 Not Found", "text/plain"),
    }
}

pub fn getAssetStr(path: &str) -> (&'static str, &'static str) {
    match path {
        "index.html" => (include_str!("../src/assets/index.html"), "text/html; charset=utf-8"),
        "assets/index.css" | "assets/index-BQHUhsXK.css" => (include_str!("../src/assets/assets/index-BQHUhsXK.css"), "text/css; charset=utf-8"),
        "assets/index.js" | "assets/index-C6mVjCLc.js" => (include_str!("../src/assets/assets/index-C6mVjCLc.js"), "application/javascript; charset=utf-8"),
        _ => ("404 Not Found", "text/plain"),
    }
}