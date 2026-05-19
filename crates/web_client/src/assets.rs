/// Embed HTML, CSS, and JS assets as const strings.
/// Used by main.rs custom protocol "app://" to serve embedded assets.

#[allow(dead_code)]
pub fn get_asset_str(path: &str) -> (&'static str, &'static str) {
    match path.trim_start_matches('/') {
        "index.html" => (include_str!("../src/assets/index.html"), "text/html; charset=utf-8"),
        "assets/index.css" | "style.css" => (include_str!("../src/assets/assets/index.css"), "text/css; charset=utf-8"),
        "assets/index.js" | "app.js" => (include_str!("../src/assets/assets/index.js"), "application/javascript; charset=utf-8"),
        _ => ("404 Not Found", "text/plain"),
    }
}

#[allow(non_snake_case)]
pub fn getAsset(path: &str) -> (&'static [u8], &'static str) {
    let (s, mime) = get_asset_str(path);
    (s.as_bytes(), mime)
}