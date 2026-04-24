use std::time::Duration;

pub async fn wait_for_healthy<F, Fut>(mut check: F, timeout: Duration, interval: Duration) -> anyhow::Result<bool>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;

    while tokio::time::Instant::now() < deadline {
        if check().await {
            return Ok(true);
        }
        tokio::time::sleep(interval).await;
    }

    Ok(false)
}