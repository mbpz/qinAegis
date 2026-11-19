//! Production UI is embedded in the service; no Vite or source checkout is needed at runtime.
pub fn get(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/" | "/index.html" => Some((
            include_str!("assets/index.html"),
            "text/html; charset=utf-8",
        )),
        "/assets/index.css" => Some((
            include_str!("assets/assets/index.css"),
            "text/css; charset=utf-8",
        )),
        "/assets/index.js" => Some((
            include_str!("assets/assets/index.js"),
            "application/javascript; charset=utf-8",
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_resources_are_not_the_application_shell() {
        assert!(super::get("/").unwrap().0.contains("<html"));
        assert!(super::get("/api/unknown").is_none());
        assert!(super::get("/assets/missing.js").is_none());
        assert!(super::get("/../config.toml").is_none());
    }
}
