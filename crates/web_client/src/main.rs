//! QinAegis local Web service and user-session launcher.
mod app;
mod case_asset_api;
mod assets;
mod contract_api;
mod dispatch;
mod environment;
mod lifecycle;
mod model_api;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lifecycle::run(std::env::args().skip(1).collect()).await
}
