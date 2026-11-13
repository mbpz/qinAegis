fn main() {
    println!(
        "cargo:rustc-env=APP_BUILD_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
    for asset in [
        "src/assets/index.html",
        "src/assets/assets/index.js",
        "src/assets/assets/index.css",
    ] {
        println!("cargo:rerun-if-changed={asset}");
        if !std::path::Path::new(asset).is_file() {
            panic!("Missing production UI resource {asset}. Run npm ci && npm run build in crates/web_client/ui before building the service.");
        }
    }
}
