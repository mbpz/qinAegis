use clap::Parser;

mod commands;
mod oauth_server;

#[derive(Parser, Debug)]
#[command(name = "qinAegis")]
#[command(version = "0.1.0")]
enum Cmd {
    Init,
    Config,
    Explore,
    Generate,
    Run,
    Report,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = Cmd::parse();
    match cmd {
        Cmd::Init => commands::init::run_init(
            std::env::var("NOTION_CLIENT_ID").unwrap_or_default(),
            std::env::var("NOTION_CLIENT_SECRET").unwrap_or_default(),
        )
        .await?,
        Cmd::Config => println!("config"),
        Cmd::Explore => println!("explore"),
        Cmd::Generate => println!("generate"),
        Cmd::Run => println!("run"),
        Cmd::Report => println!("report"),
    }
    Ok(())
}