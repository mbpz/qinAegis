use clap::Parser;

mod commands;
mod oauth_server;
mod config;

#[derive(Parser, Debug)]
#[command(name = "qinAegis")]
#[command(version = "0.1.0")]
enum Cmd {
    /// Setup or reconfigure QinAegis (interactive)
    Setup,
    /// Initialize Notion workspace and databases
    Init,
    /// List projects from Notion
    ListProjects,
    /// Show current configuration
    Config,
    Explore {
        #[arg(long)]
        project: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, default_value = "3")]
        depth: u32,
    },
    Generate {
        #[arg(long)]
        project: String,
        #[arg(long)]
        requirement: String,
        #[arg(long)]
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
        Cmd::Setup => {
            let config = config::prompt_for_config()?;
            config.save()?;
            println!("Configuration saved to {}", config::Config::config_path().display());
        }
        Cmd::Init => commands::init::run_init_and_setup().await?,
        Cmd::ListProjects => commands::notion::list_projects().await?,
        Cmd::Config => {
            match config::Config::load()? {
                Some(cfg) => {
                    println!("Current configuration:");
                    println!("  LLM: {} (provider: {}, model: {})",
                        if cfg.is_llm_configured() { "configured" } else { "NOT configured" },
                        cfg.llm.provider, cfg.llm.model);
                }
                None => {
                    println!("No configuration found. Run 'qinAegis setup' first.");
                }
            }
        }
        Cmd::Explore { project, url, depth } => commands::explore::run_explore(&project, url, depth).await?,
        Cmd::Generate { project, requirement, spec } => {
            commands::generate::run_generate(&project, &requirement, std::path::Path::new(&spec)).await?
        }
        Cmd::Run { project, test_type, concurrency } => {
            commands::run::run_tests(&project, &test_type, concurrency).await?
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
