//! QinAegis local Web service and user-session launcher.
mod app;
mod assets;
mod contract_api;
mod model_api;
mod dispatch;
mod lifecycle;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lifecycle::run(std::env::args().skip(1).collect()).await
}
