use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub name: String,
    pub test_case_id: String,
    pub status: TestResultStatus,
    pub duration_ms: u64,
    pub run_at: String,
    pub environment: Environment,
    pub report_url: Option<String>,
    pub screenshot_urls: Vec<String>,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub metrics_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestResultStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Dev,
    Staging,
    Prod,
}

impl TestResult {
    pub fn from_notion_page(page: &serde_json::Value) -> Option<Self> {
        let id = page["id"].as_str()?.to_string();
        let name = page["properties"]["name"]["title"]
            .as_array()?
            .first()?
            .get("text")?
            .get("content")?
            .as_str()?
            .to_string();

        let test_case_id = page["properties"]["test_case"]["relation"]
            .as_array()?
            .first()?
            .get("id")?
            .as_str()?
            .to_string();

        let status_str = page["properties"]["status"]["select"]["name"]
            .as_str()
            .unwrap_or("failed");
        let status = match status_str {
            "passed" => TestResultStatus::Passed,
            "skipped" => TestResultStatus::Skipped,
            "error" => TestResultStatus::Error,
            _ => TestResultStatus::Failed,
        };

        let duration_ms = page["properties"]["duration_ms"]["number"]
            .as_u64()
            .unwrap_or(0);

        let run_at = page["properties"]["run_at"]["date"]["start"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let env_str = page["properties"]["environment"]["select"]["name"]
            .as_str()
            .unwrap_or("dev");
        let environment = match env_str {
            "staging" => Environment::Staging,
            "prod" => Environment::Prod,
            _ => Environment::Dev,
        };

        let report_url = page["properties"]["report_url"]["url"]
            .as_str()
            .map(String::from);

        let screenshot_urls = page["properties"]["screenshot_url"]["files"]
            .as_array()?
            .iter()
            .filter_map(|v| v["file"]["url"].as_str().map(String::from))
            .collect();

        let error_message = page["properties"]["error_message"]["rich_text"]
            .as_array()?
            .first()?
            .get("text")?
            .get("content")?
            .as_str()
            .map(String::from);

        let retry_count = page["properties"]["retry_count"]["number"]
            .as_u64()
            .unwrap_or(0) as u32;

        let metrics_json = page["properties"]["metrics_json"]["code"]
            .as_str()
            .map(String::from);

        Some(TestResult {
            id,
            name,
            test_case_id,
            status,
            duration_ms,
            run_at,
            environment,
            report_url,
            screenshot_urls,
            error_message,
            retry_count,
            metrics_json,
        })
    }

    pub fn to_notion_properties(&self) -> serde_json::Value {
        let status_str = match self.status {
            TestResultStatus::Passed => "Passed",
            TestResultStatus::Failed => "Failed",
            TestResultStatus::Skipped => "Skipped",
            TestResultStatus::Error => "Error",
        };

        let env_str = match self.environment {
            Environment::Dev => "Dev",
            Environment::Staging => "Staging",
            Environment::Prod => "Prod",
        };

        serde_json::json!({
            "name": { "title": [{ "text": { "content": &self.name } }] },
            "test_case": { "relation": [{ "id": &self.test_case_id }] },
            "status": { "select": { "name": status_str } },
            "duration_ms": { "number": self.duration_ms as f64 },
            "run_at": { "date": { "start": &self.run_at } },
            "environment": { "select": { "name": env_str } },
            "report_url": self.report_url.as_ref().map(|u| serde_json::json!({ "url": u })).unwrap_or(serde_json::json!({ "url": null })),
            "error_message": self.error_message.as_ref().map(|e| serde_json::json!({ "rich_text": [{ "text": { "content": e } }] })).unwrap_or(serde_json::json!({ "rich_text": [] })),
            "retry_count": { "number": self.retry_count as f64 },
            "metrics_json": self.metrics_json.as_ref().map(|m| serde_json::json!({ "code": { "language": "json", "content": m } })).unwrap_or(serde_json::json!({ "code": null }))
        })
    }
}
