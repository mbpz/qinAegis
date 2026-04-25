use clap::Parser;

mod commands;
mod oauth_server;

#[derive(Parser, Debug)]
#[command(name = "qinAegis")]
#[command(version = "0.1.0")]
enum Cmd {
    Init,
    InitDb,
    ListProjects,
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
    Run {
        #[arg(long)]
        project: String,
        #[arg(long, default_value = "smoke")]
        test_type: String,
        #[arg(long, default_value = "4")]
        concurrency: usize,
    },
    Report,
    Performance {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "10")]
        threshold: f64,
    },
    Stress {
        #[arg(long)]
        target: String,
        #[arg(long, default_value = "100")]
        users: u32,
        #[arg(long, default_value = "10")]
        spawn_rate: u32,
        #[arg(long, default_value = "60")]
        duration: u32,
    },
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
        Cmd::InitDb => commands::notion::init_databases().await?,
        Cmd::ListProjects => commands::notion::list_projects().await?,
        Cmd::Config => println!("config"),
        Cmd::Explore { url, depth } => commands::explore::run_explore(url, depth).await?,
        Cmd::Generate { requirement, spec } => {
            commands::generate::run_generate(&requirement, spec.as_ref()).await?
        }
        Cmd::Run { project, test_type, concurrency } => {
            commands::run::run_tests(&test_type, &project, concurrency).await?
        }
        Cmd::Report => println!("report"),
        Cmd::Performance { url, threshold } => {
            commands::performance::run_performance(&url, threshold).await?
        }
        Cmd::Stress { target, users, spawn_rate, duration } => {
            commands::performance::run_stress(qin_aegis_core::StressTestConfig::new(&target, users, spawn_rate, duration)).await?
        }
    }
    Ok(())
}
