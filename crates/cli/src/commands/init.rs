// crates/cli/src/commands/init.rs
use crate::oauth_server::OAuthServer;
use qinAegis_notion::auth::NotionAuth;

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
    qinAegis_notion::auth::store_notion_token(&token_resp.access_token)?;

    println!("Connected to Notion workspace: {}", token_resp.workspace_name);
    Ok(())
}