// crates/cli/src/commands/init.rs
use crate::oauth_server::OAuthServer;
use qin_aegis_notion::auth::{get_notion_token, store_notion_token, NotionAuth};
use qin_aegis_notion::database::{
    NotionClient, PROJECTS_DB_SPEC, REQUIREMENTS_DB_SPEC, TEST_CASES_DB_SPEC, TEST_RESULTS_DB_SPEC,
};
use std::path::PathBuf;

pub async fn run_init(client_id: String, client_secret: String) -> anyhow::Result<()> {
    // 1. Start OAuth callback server
    let port = 54321;
    let server = OAuthServer::new(port);

    // 2. Open browser for authorization
    let auth = NotionAuth::new(client_id.clone(), port);
    let url = auth.authorization_url();

    println!("Opening browser for Notion authorization...");
    open::that(&url)?;

    // 3. Create channel and start server in background task
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start(tx).await {
            eprintln!("OAuth server error: {}", e);
        }
    });

    // 4. Wait for callback
    let result = rx.recv().await;
    server_handle.abort();

    let code = match result {
        Some(Ok(c)) => c,
        Some(Err(e)) => anyhow::bail!("OAuth error: {}", e),
        None => anyhow::bail!("OAuth server closed without code"),
    };

    // 5. Exchange code for token
    let token_resp = auth.exchange_code(&code, &client_secret).await?;

    // 6. Store in Keychain
    store_notion_token(&token_resp.access_token)?;

    println!("Connected to Notion workspace: {}", token_resp.workspace_name);
    Ok(())
}

pub async fn run_init_and_setup(client_id: String, client_secret: String) -> anyhow::Result<()> {
    // 1. Run OAuth flow (reuses run_init logic)
    run_init(client_id, client_secret).await?;

    // 2. Get token from keychain
    let token = match get_notion_token()? {
        Some(t) => t,
        None => anyhow::bail!("no token stored after OAuth"),
    };

    let client = NotionClient::new(&token);

    // 3. Create parent page to host the databases
    println!("Creating QinAegis workspace page...");
    let parent_page_id = client
        .create_page("QinAegis", "workspace")
        .await
        .map_err(|e| anyhow::anyhow!("failed to create parent page: {}", e))?;
    println!("  ✓ Workspace page created: {}", parent_page_id);

    // 4. Create all 4 databases
    println!("Creating 4 databases...");

    let projects_id = client
        .create_database(&parent_page_id, &PROJECTS_DB_SPEC)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create Projects DB: {}", e))?;
    println!("  ✓ Projects database created: {}", projects_id);

    let requirements_id = client
        .create_database(&parent_page_id, &REQUIREMENTS_DB_SPEC)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create Requirements DB: {}", e))?;
    println!("  ✓ Requirements database created: {}", requirements_id);

    let test_cases_id = client
        .create_database(&parent_page_id, &TEST_CASES_DB_SPEC)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create TestCases DB: {}", e))?;
    println!("  ✓ TestCases database created: {}", test_cases_id);

    let test_results_id = client
        .create_database(&parent_page_id, &TEST_RESULTS_DB_SPEC)
        .await
        .map_err(|e| anyhow::anyhow!("failed to create TestResults DB: {}", e))?;
    println!("  ✓ TestResults database created: {}", test_results_id);

    // 5. Save DB IDs to config
    save_notion_db_ids(&NotionDbIds {
        projects_id,
        requirements_id,
        test_cases_id,
        test_results_id,
    })?;

    println!("\n✓ Workspace initialized successfully!");
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NotionDbIds {
    projects_id: String,
    requirements_id: String,
    test_cases_id: String,
    test_results_id: String,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("qinAegis")
        .join("config.toml")
}

fn save_notion_db_ids(ids: &NotionDbIds) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(ids)?;
    std::fs::write(&path, content)?;
    println!("  ✓ Config saved to {}", path.display());
    Ok(())
}
