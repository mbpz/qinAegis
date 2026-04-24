use crate::NotionClient;
use serde_json::{json, Value};
use std::path::Path;

pub struct NotionWriter<'a> {
    client: &'a NotionClient,
    db_id: &'a str,
}

impl<'a> NotionWriter<'a> {
    pub fn new(client: &'a NotionClient, test_results_db_id: &'a str) -> Self {
        Self { client, db_id: test_results_db_id }
    }

    pub async fn write_result(
        &self,
        _case_id: &str,
        case_name: &str,
        test_case_relation_id: &str,
        result: &Value,
        run_id: &str,
        report_url: Option<&str>,
    ) -> anyhow::Result<String> {
        let passed = result["passed"].as_bool().unwrap_or(false);
        let status = if passed { "Passed" } else { "Failed" };
        let name = format!("{}-{}", case_name, run_id);
        let duration_ms = result["duration_ms"].as_u64().unwrap_or(0) as f64;
        let error_message = result["error_message"].as_str();

        let body = json!({
            "parent": { "database_id": self.db_id },
            "properties": {
                "name": { "title": [{ "text": { "content": name } }] },
                "test_case": { "relation": [{ "id": test_case_relation_id }] },
                "status": { "select": { "name": status } },
                "duration_ms": { "number": duration_ms },
                "run_at": { "date": { "start": chrono::Utc::now().to_rfc3339() } },
                "error_message": {
                    "rich_text": error_message
                        .map(|e| json!([{ "text": { "content": e } }]))
                        .unwrap_or(json!([]))
                },
                "report_url": report_url.map(|u| json!({ "url": u })).unwrap_or(json!(null)),
            }
        });

        let resp = self.client.post("pages", &body).await?;
        let json_resp: Value = resp.json().await?;
        json_resp["id"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no page id in response"))
    }

    pub async fn batch_write_results(
        &self,
        results: Vec<WriteRequest>,
        run_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut page_ids = Vec::new();
        for req in results {
            let page_id = self.write_result(
                &req.case_id,
                &req.case_name,
                &req.test_case_relation_id,
                &req.result,
                run_id,
                req.report_url.as_deref(),
            ).await?;
            page_ids.push(page_id);
        }
        Ok(page_ids)
    }

    pub async fn upload_file(&self, page_id: &str, file_path: &Path) -> anyhow::Result<String> {
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "report.html".to_string());

        let client = reqwest::Client::new();
        let file_bytes = tokio::fs::read(file_path).await?;

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name.to_string());

        let form = reqwest::multipart::Form::new()
            .text("name", file_name)
            .part("file", part);

        let resp = client
            .post(format!("https://api.notion.com/v1/pages/{}/attachments", page_id))
            .bearer_auth(&self.client.token)
            .header("Notion-Version", "2022-06-28")
            .multipart(form)
            .send()
            .await?;

        let json: Value = resp.json().await?;
        let file_url = json["file"]["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("no file_url in attachment response"))?;

        Ok(file_url)
    }
}

#[derive(Debug)]
pub struct WriteRequest {
    pub case_id: String,
    pub case_name: String,
    pub test_case_relation_id: String,
    pub result: Value,
    pub report_url: Option<String>,
}
