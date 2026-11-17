//! QinAegis local Web service and user-session launcher.
mod app;
mod assets;
mod case_asset_api;
mod config_migration_api;
mod contract_api;
mod flow_api;
mod dispatch;
mod environment;
mod lifecycle;
mod model_api;
mod report_maintenance;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lifecycle::run(std::env::args().skip(1).collect()).await
}
