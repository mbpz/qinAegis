// crates/cli/src/oauth_server.rs
use axum::{
    extract::Query,
    response::Html,
    routing::get,
    Router,
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

#[derive(Deserialize, Debug)]
pub struct OAuthCallback {
    code: Option<String>,
    error: Option<String>,
}

pub struct OAuthServer {
    port: u16,
}

impl OAuthServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub async fn start(&self, tx: Sender<Result<String, String>>) -> anyhow::Result<()> {
        let app = Router::new().route(
            "/callback",
            get(|Query(params): Query<OAuthCallback>| async move {
                if let Some(error) = params.error {
                    let _ = tx.send(Err(error)).await;
                    return Html("<h1>Authorization failed</h1>".to_string());
                }
                if let Some(code) = params.code {
                    let _ = tx.send(Ok(code.clone())).await;
                    return Html("<h1>Authorization successful! Close this window.</h1>".to_string());
                }
                Html("<h1>Missing code parameter</h1>".to_string())
            }),
        );

        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}