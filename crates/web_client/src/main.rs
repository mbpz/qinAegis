//! QinAegis local Web service and user-session launcher.
mod app;
mod assets;
mod case_asset_api;
mod config_migration_api;
mod contract_api;
mod flow_api;
mod flow_execution_api;
mod dispatch;
mod environment;
mod lifecycle;
mod model_api;
mod generation_evidence;
mod report_maintenance;
mod runtime_api;
mod server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lifecycle::run(std::env::args().skip(1).collect()).await
}
