// crates/cli/src/commands/init.rs
use crate::config::{Config, prompt_for_config};
use qin_aegis_core::storage::LocalStorage;

pub async fn run_init_and_setup() -> anyhow::Result<()> {
    // 1. Check if config exists
    match Config::load()? {
        Some(c) if c.is_complete() => {
            println!("Configuration already complete.");
            println!("  LLM: {} ({})", c.llm.model, c.llm.base_url);
            println!("\nTo reconfigure, delete ~/.qinAegis/config.toml and run init again.");
            return Ok(());
        }
        Some(_) => {
            println!("Incomplete configuration detected. Running setup...");
        }
        None => {
            println!("No configuration found. Creating new setup...");
        }
    }

    // 2. Prompt for config
    let config = prompt_for_config()?;
    config.save()?;

    // 3. Initialize projects directory
    let projects_dir = LocalStorage::projects_dir();
    std::fs::create_dir_all(&projects_dir)?;

    println!("\n✓ Initialization complete!");
    println!("  Config: ~/.qinAegis/config.toml");
    println!("  Projects: ~/.qinAegis/projects/");
    println!("\nNext steps:");
    println!("  qinAegis project add <name>  # Add a project");
    println!("  qinAegis explore <project>   # Explore a URL");

    Ok(())
}
