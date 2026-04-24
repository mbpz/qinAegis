use clap::Parser;

mod commands;
mod oauth_server;

#[derive(Parser, Debug)]
#[command(name = "qinAegis")]
#[command(version = "0.1.0")]
enum Cmd {
    Init,
    Config,
    Explore {
        #[arg(long)]
        url: Vec<String>,
        #[arg(long, default_value = "3")]
        depth: u32,
    },
    Generate {
        #[arg(long)]
        requirement: String,
        #[arg(long, default_value = "~/.local/share/qinAegis/exploration/spec.md")]
        spec: String,
    },
    Run,
    Report,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = Cmd::parse();
    match cmd {
        Cmd::Init => commands::init::run_init_and_setup(
            std::env::var("NOTION_CLIENT_ID").unwrap_or_default(),
            std::env::var("NOTION_CLIENT_SECRET").unwrap_or_default(),
        )
        .await?,
        Cmd::Config => println!("config"),
        Cmd::Explore { url, depth } => commands::explore::run_explore(url, depth).await?,
        Cmd::Generate { requirement, spec } => {
            commands::generate::run_generate(&requirement, spec.as_ref()).await?
        }
        Cmd::Run => println!("run"),
        Cmd::Report => println!("report"),
    }
    Ok(())
}