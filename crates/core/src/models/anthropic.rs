//! Native Messages wire protocol. This adapter does not drive Midscene vision.
use super::*;
use serde_json::{json, Value};

pub(super) struct AnthropicFactory;
impl ModelAdapterFactory for AnthropicFactory {
    fn adapter(&self) -> &str {
        "anthropic_messages"
    }
    fn build(&self, profile: &ModelProfile) -> Result<ArcLlmClient> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| anyhow::anyhow!("cannot initialize Messages HTTP client"))?;
        Ok(ArcLlmClient::new(MessagesClient {
            profile: profile.clone(),
            client,
        }))
    }
}
struct MessagesClient {
    profile: ModelProfile,
    client: reqwest::Client,
}
fn rejected(message: &'static str) -> LlmError {
    LlmError::Api(message.into())
}
fn request(
    profile: &ModelProfile,
    messages: &[Message],
    options: &ChatOptions,
) -> std::result::Result<Value, LlmError> {
    if options.vision == Some(true) || options.temperature.is_some() {
        return Err(LlmError::UnsupportedCapability(
            "Messages adapter does not support image input or temperature",
        ));
    }
    let max_tokens = options.max_tokens.unwrap_or(4096);
    if max_tokens == 0 || max_tokens > 65536 {
        return Err(rejected("invalid Messages output token limit"));
    }
    let mut system = Vec::new();
    if let Some(text) = &options.system_prompt {
        system.push(text.clone());
    }
    let mut conversation = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "system" if conversation.is_empty() => system.push(message.content.clone()),
            "user" | "assistant" => conversation.push(
                json!({"role":message.role,"content":[{"type":"text","text":message.content}]}),
            ),
            _ => {
                return Err(rejected(
                    "unsupported Messages role or misplaced system message",
                ))
            }
        }
    }
    if conversation.is_empty()
        || conversation[0]["role"] != "user"
        || conversation.last().unwrap()["role"] != "user"
    {
        return Err(rejected(
            "Messages conversation must start and end with user",
        ));
    }
    let mut body = json!({"model":profile.model,"max_tokens":max_tokens,"messages":conversation});
    if !system.is_empty() {
        body["system"] = Value::String(system.join("\n\n"));
    }
    if let Some(schema) = &options.json_schema {
        let schema: Value =
            serde_json::from_str(schema).map_err(|_| rejected("invalid Messages JSON schema"))?;
        if !schema.is_object() {
            return Err(rejected("Messages JSON schema must be an object"));
        }
        body["output_config"] = json!({"format":{"type":"json_schema","schema":schema}});
    }
    Ok(body)
}
fn output(bytes: &[u8], structured: bool) -> std::result::Result<String, LlmError> {
    let response: Value =
        serde_json::from_slice(bytes).map_err(|_| rejected("invalid Messages response"))?;
    if response["type"] != "message"
        || response["role"] != "assistant"
        || response["stop_reason"] != "end_turn"
    {
        return Err(rejected("Messages response did not complete normally"));
    }
    let blocks = response["content"]
        .as_array()
        .ok_or_else(|| rejected("missing Messages content"))?;
    let mut result = String::new();
    for block in blocks {
        if block["type"] != "text" {
            return Err(rejected("unsupported Messages content block"));
        }
        result.push_str(
            block["text"]
                .as_str()
                .ok_or_else(|| rejected("invalid Messages text"))?,
        );
    }
    if result.trim().is_empty() {
        return Err(rejected("empty Messages response"));
    }
    if structured && serde_json::from_str::<Value>(&result).is_err() {
        return Err(rejected("invalid Messages structured output"));
    }
    Ok(result)
}
#[async_trait]
impl LlmClient for MessagesClient {
    async fn chat(&self, messages: &[Message]) -> std::result::Result<String, LlmError> {
        self.chat_with_options(messages, ChatOptions::default())
            .await
    }
    async fn chat_with_options(
        &self,
        messages: &[Message],
        options: ChatOptions,
    ) -> std::result::Result<String, LlmError> {
        if self.profile.api_key.is_empty() {
            return Err(LlmError::NoApiKey);
        }
        let body = request(&self.profile, messages, &options)?;
        let mut response = self
            .client
            .post(format!(
                "{}/messages",
                self.profile.base_url.trim_end_matches('/')
            ))
            .header("x-api-key", &self.profile.api_key)
            .header("anthropic-version", "2026-10-21")
            .json(&body)
            .send()
            .await
            .map_err(|_| LlmError::ModelFailure("Messages transport failure"))?;
        let status = response.status();
        if status.as_u16() == 429 || status.is_server_error() {
            return Err(LlmError::ModelFailure(
                "Messages quota or service unavailable",
            ));
        }
        if !status.is_success() {
            return Err(rejected("Messages request rejected"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| LlmError::ModelFailure("Messages response transport failure"))?
        {
            if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                return Err(rejected("Messages response exceeds limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        output(&bytes, options.json_schema.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn profile(endpoint: String) -> ModelProfile {
        ModelProfile {
            id: "messages".into(),
            label: "Messages".into(),
            adapter: "anthropic_messages".into(),
            base_url: endpoint,
            model: "claude-test".into(),
            api_key: "secret-fixture-key".into(),
            capabilities: vec!["text".into(), "json".into()],
            enabled: true,
            allow_remote: false,
            midscene_family: None,
        }
    }
    fn completed(text: &str) -> String {
        json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":text}]}).to_string()
    }
    async fn server(
        status: u16,
        body: String,
        delay: Duration,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 4096];
                let size = stream.read(&mut chunk).await.unwrap();
                if size == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..size]);
                if let Some(end) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&bytes[..end]);
                    let length = head
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|v| v.parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            tokio::time::sleep(delay).await;
            let response=format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",body.len());
            let _ = stream.write_all(response.as_bytes()).await;
            String::from_utf8(bytes).unwrap()
        });
        (endpoint, handle)
    }
    #[tokio::test]
    async fn native_wire_preserves_system_and_structured_output_contract() {
        let (endpoint, server) = server(200, completed(r#"{"ok":true}"#), Duration::ZERO).await;
        let client = AnthropicFactory.build(&profile(endpoint)).unwrap();
        let schema = r#"{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}"#;
        let result = client
            .chat_with_options(
                &[
                    Message::system("original assertions"),
                    Message::user("requirement"),
                ],
                ChatOptions::new()
                    .with_json_schema(schema)
                    .with_max_tokens(32),
            )
            .await
            .unwrap();
        assert_eq!(result, r#"{"ok":true}"#);
        let received = server.await.unwrap();
        let (headers, body) = received.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(headers.contains("x-api-key: secret-fixture-key"));
        assert!(headers.contains("anthropic-version: 2026-10-21"));
        assert!(!headers.to_ascii_lowercase().contains("authorization:"));
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["system"], "original assertions");
        assert_eq!(body["messages"][0]["content"][0]["text"], "requirement");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(
            body["output_config"]["format"]["schema"],
            serde_json::from_str::<Value>(schema).unwrap()
        );
    }
    #[tokio::test]
    async fn wire_rejections_quota_truncation_and_bounds_are_safe() {
        for (status, body, retry) in [
            (401, "secret-fixture-key".into(), false),
            (429, "secret-fixture-key".into(), true),
            (529, "overloaded".into(), true),
            (302, "redirect".into(), false),
            (200, "malformed secret-fixture-key".into(), false),
            (200, "x".repeat(4 * 1024 * 1024 + 1), false),
        ] {
            let (endpoint, server) = server(status, body, Duration::ZERO).await;
            let error = AnthropicFactory
                .build(&profile(endpoint))
                .unwrap()
                .chat(&[Message::user("hi")])
                .await
                .err()
                .unwrap();
            assert_eq!(matches!(error, LlmError::ModelFailure(_)), retry);
            assert!(!error.to_string().contains("secret-fixture-key"));
            server.await.unwrap();
        }
        for reason in ["max_tokens", "refusal", "tool_use", "pause_turn", ""] {
            let bytes=json!({"type":"message","role":"assistant","stop_reason":reason,"content":[{"type":"text","text":"partial"}]}).to_string();
            assert!(output(bytes.as_bytes(), false).is_err());
        }
        assert!(output(completed("not json").as_bytes(), true).is_err());
        assert!(output(br#"{"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"tool_use"}]}"#,false).is_err());
    }
    #[tokio::test]
    async fn approved_cross_protocol_fallback_and_deadline_are_bounded() {
        let (first, s1) = server(429, "quota".into(), Duration::ZERO).await;
        let (second, s2) = server(
            200,
            json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"ok"}}]}).to_string(),
            Duration::ZERO,
        )
        .await;
        let trace = Arc::new(ModelCallTrace::default());
        let route = RegisteredClient {
            clients: vec![
                (
                    "primary".into(),
                    AnthropicFactory.build(&profile(first)).unwrap(),
                ),
                (
                    "fallback".into(),
                    OpenAiCompatibleFactory.build(&profile(second)).unwrap(),
                ),
            ],
            capabilities: BTreeSet::from(["text".into()]),
            trace: Some(trace.clone()),
        };
        assert_eq!(
            route
                .chat_with_budget(
                    &[Message::user("hi")],
                    ChatOptions::default(),
                    Duration::from_secs(2)
                )
                .await
                .unwrap(),
            "ok"
        );
        assert_eq!(trace.snapshot().unwrap().len(), 2);
        s1.await.unwrap();
        assert!(s2
            .await
            .unwrap()
            .starts_with("POST /v1/chat/completions HTTP/1.1"));
        let (endpoint, server) = server(200, completed("late"), Duration::from_secs(1)).await;
        let route = RegisteredClient {
            clients: vec![(
                "slow".into(),
                AnthropicFactory.build(&profile(endpoint)).unwrap(),
            )],
            capabilities: BTreeSet::from(["text".into()]),
            trace: Some(trace.clone()),
        };
        assert!(route
            .chat_with_budget(
                &[Message::user("hi")],
                ChatOptions::default(),
                Duration::from_millis(30)
            )
            .await
            .is_err());
        assert_eq!(
            trace.snapshot().unwrap().last().unwrap().outcome,
            ModelCallOutcome::TimedOut
        );
        server.abort();
    }
    #[test]
    fn profile_and_requests_reject_unsupported_options_before_network() {
        let root = tempfile::tempdir().unwrap();
        let registry = ModelRegistry::new(root.path().join("models.json"));
        let mut p = profile("http://127.0.0.1:9/v1".into());
        registry.upsert(0, p.clone(), false).unwrap();
        assert!(registry
            .set_route(
                1,
                "execute",
                TaskModelRoute {
                    primary: "messages".into(),
                    fallbacks: vec![],
                },
            )
            .is_err());
        assert!(registry.resolve_for_project(None, "execute", &[]).is_err());
        p.capabilities.push("vision".into());
        p.midscene_family = Some("claude".into());
        assert!(registry.upsert(1, p.clone(), false).is_err());
        assert!(request(
            &p,
            &[Message::user("x"), Message::system("late")],
            &ChatOptions::default()
        )
        .is_err());
        assert!(request(
            &p,
            &[Message::user("x")],
            &ChatOptions::new().with_temperature(0.1)
        )
        .is_err());
        assert!(request(
            &p,
            &[Message::user("x")],
            &ChatOptions::new().with_json_schema("invalid")
        )
        .is_err());
        assert!(request(
            &p,
            &[Message::user("x")],
            &ChatOptions::new().with_max_tokens(0)
        )
        .is_err());
    }
}
