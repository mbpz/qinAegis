use std::sync::LazyLock;

use super::models::{DatabaseSpec, PropertySchema};

pub struct NotionClient {
    pub token: String,
}

impl NotionClient {
    pub fn new(token: &str) -> Self {
        Self { token: token.to_string() }
    }

    pub async fn post(&self, endpoint: &str, body: &serde_json::Value) -> anyhow::Result<reqwest::Response> {
        let client = reqwest::Client::new();
        client
            .post(format!("https://api.notion.com/v1/{}", endpoint))
            .bearer_auth(&self.token)
            .header("Notion-Version", "2022-06-28")
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("notion api error: {}", e))
    }

    pub async fn create_database(&self, parent_id: &str, spec: &DatabaseSpec) -> anyhow::Result<String> {
        let properties: serde_json::Value = spec
            .properties
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    serde_json::json!({ p.property_type.clone(): {} }),
                )
            })
            .collect();

        let body = serde_json::json!({
            "parent": { "page_id": parent_id },
            "title": [{ "text": { "content": &spec.name } }],
            "properties": properties
        });

        let resp = self
            .post("databases", &body)
            .await?
            .json::<serde_json::Value>()
            .await?;

        resp["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no database id in response"))
    }

    pub async fn create_page(&self, title: &str, parent_id: &str) -> anyhow::Result<String> {
        let body = serde_json::json!({
            "parent": { "page_id": parent_id },
            "properties": {
                "title": {
                    "title": [{ "text": { "content": title } }]
                }
            }
        });
        let resp = self.post("pages", &body).await?;
        let json: serde_json::Value = resp.json().await?;
        json["id"].as_str().map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no page id in response"))
    }
}

pub static PROJECTS_DB_SPEC: LazyLock<DatabaseSpec, fn() -> DatabaseSpec> = LazyLock::new(|| DatabaseSpec {
    name: String::from("Projects"),
    properties: vec![
        PropertySchema { name: String::from("name"), property_type: String::from("title") },
        PropertySchema { name: String::from("url"), property_type: String::from("url") },
        PropertySchema { name: String::from("tech_stack"), property_type: String::from("multi_select") },
        PropertySchema { name: String::from("status"), property_type: String::from("select") },
    ],
});

pub static REQUIREMENTS_DB_SPEC: LazyLock<DatabaseSpec, fn() -> DatabaseSpec> = LazyLock::new(|| DatabaseSpec {
    name: String::from("Requirements"),
    properties: vec![
        PropertySchema { name: String::from("name"), property_type: String::from("title") },
        PropertySchema { name: String::from("project"), property_type: String::from("relation") },
        PropertySchema { name: String::from("description"), property_type: String::from("rich_text") },
        PropertySchema { name: String::from("priority"), property_type: String::from("select") },
        PropertySchema { name: String::from("status"), property_type: String::from("select") },
    ],
});

pub static TEST_CASES_DB_SPEC: LazyLock<DatabaseSpec, fn() -> DatabaseSpec> = LazyLock::new(|| DatabaseSpec {
    name: String::from("TestCases"),
    properties: vec![
        PropertySchema { name: String::from("name"), property_type: String::from("title") },
        PropertySchema { name: String::from("requirement"), property_type: String::from("relation") },
        PropertySchema { name: String::from("type"), property_type: String::from("select") },
        PropertySchema { name: String::from("priority"), property_type: String::from("select") },
        PropertySchema { name: String::from("status"), property_type: String::from("select") },
        PropertySchema { name: String::from("yaml_script"), property_type: String::from("code") },
        PropertySchema { name: String::from("expected_result"), property_type: String::from("rich_text") },
        PropertySchema { name: String::from("tags"), property_type: String::from("multi_select") },
    ],
});

pub static TEST_RESULTS_DB_SPEC: LazyLock<DatabaseSpec, fn() -> DatabaseSpec> = LazyLock::new(|| DatabaseSpec {
    name: String::from("TestResults"),
    properties: vec![
        PropertySchema { name: String::from("name"), property_type: String::from("title") },
        PropertySchema { name: String::from("test_case"), property_type: String::from("relation") },
        PropertySchema { name: String::from("status"), property_type: String::from("select") },
        PropertySchema { name: String::from("duration_ms"), property_type: String::from("number") },
        PropertySchema { name: String::from("run_at"), property_type: String::from("date") },
        PropertySchema { name: String::from("report_url"), property_type: String::from("url") },
        PropertySchema { name: String::from("error_message"), property_type: String::from("rich_text") },
    ],
});