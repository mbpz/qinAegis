use qin_aegis_core::Explorer;

pub async fn run_explore(seed_urls: Vec<String>, max_depth: u32) -> anyhow::Result<()> {
    println!("Starting project exploration...");
    println!("Seed URLs: {:?}", seed_urls);
    println!("Max depth: {}", max_depth);

    let mut explorer = Explorer::new().await?;

    let mut all_pages = vec![];
    let mut all_markdown = String::from("# 项目规格书\n\n");

    for url in &seed_urls {
        println!("Exploring {}", url);
        let result = explorer.explore(url, max_depth).await?;
        all_pages.extend(result.pages);
        all_markdown.push_str(&result.markdown);
    }

    let output_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("qinAegis")
        .join("exploration");

    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("spec.md");
    std::fs::write(&output_path, &all_markdown)?;

    println!("\n✓ Exploration complete: {} pages", all_pages.len());
    println!("✓ Spec saved to: {}", output_path.display());

    explorer.shutdown().await?;
    Ok(())
}
