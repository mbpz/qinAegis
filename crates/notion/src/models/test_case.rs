use serde::{Deserialize, Serialize};
use crate::models::Priority;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub requirement_id: String,
    pub test_type: TestType,
    pub priority: Priority,
    pub status: TestCaseStatus,
    pub yaml_script: String,
    pub expected_result: String,
    pub tags: Vec<String>,
    pub created_by: CreatedBy,
    pub reviewed_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestType {
    Smoke,
    Functional,
    Performance,
    Stress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestCaseStatus {
    Draft,
    Approved,
    Rejected,
    Deprecated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatedBy {
    Ai,
    Human,
}

impl TestCase {
    pub fn from_notion_page(page: &serde_json::Value) -> Option<Self> {
        let id = page["id"].as_str()?.to_string();
        let name = page["properties"]["name"]["title"]
            .as_array()?
            .first()?
            .get("text")?
            .get("content")?
            .as_str()?
            .to_string();

        let requirement_id = page["properties"]["requirement"]["relation"]
            .as_array()?
            .first()?
            .get("id")?
            .as_str()?
            .to_string();

        let test_type_str = page["properties"]["type"]["select"]["name"]
            .as_str()
            .unwrap_or("functional");
        let test_type = match test_type_str {
            "smoke" => TestType::Smoke,
            "performance" => TestType::Performance,
            "stress" => TestType::Stress,
            _ => TestType::Functional,
        };

        let priority_str = page["properties"]["priority"]["select"]["name"]
            .as_str()
            .unwrap_or("medium");
        let priority = match priority_str {
            "high" => Priority::P0,
            "low" => Priority::P2,
            _ => Priority::P1,
        };

        let status_str = page["properties"]["status"]["select"]["name"]
            .as_str()
            .unwrap_or("draft");
        let status = match status_str {
            "approved" => TestCaseStatus::Approved,
            "rejected" => TestCaseStatus::Rejected,
            "deprecated" => TestCaseStatus::Deprecated,
            _ => TestCaseStatus::Draft,
        };

        let yaml_script = page["properties"]["yaml_script"]["code"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let expected_result = page["properties"]["expected_result"]["rich_text"]
            .as_array()?
            .iter()
            .map(|v| v["text"]["content"].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("");

        let tags = page["properties"]["tags"]["multi_select"]
            .as_array()?
            .iter()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();

        let created_by_str = page["properties"]["created_by"]["select"]["name"]
            .as_str()
            .unwrap_or("ai");
        let created_by = match created_by_str {
            "human" => CreatedBy::Human,
            _ => CreatedBy::Ai,
        };

        let reviewed_by = page["properties"]["reviewed_by"]["select"]["name"]
            .as_str()
            .map(String::from);

        Some(TestCase {
            id,
            name,
            requirement_id,
            test_type,
            priority,
            status,
            yaml_script,
            expected_result,
            tags,
            created_by,
            reviewed_by,
        })
    }

    pub fn to_notion_properties(&self) -> serde_json::Value {
        let test_type_str = match self.test_type {
            TestType::Smoke => "smoke",
            TestType::Functional => "functional",
            TestType::Performance => "performance",
            TestType::Stress => "stress",
        };

        let priority_str = match self.priority {
            Priority::P0 => "high",
            Priority::P1 => "medium",
            Priority::P2 => "low",
        };

        let status_str = match self.status {
            TestCaseStatus::Draft => "draft",
            TestCaseStatus::Approved => "approved",
            TestCaseStatus::Rejected => "rejected",
            TestCaseStatus::Deprecated => "deprecated",
        };

        let created_by_str = match self.created_by {
            CreatedBy::Ai => "AI",
            CreatedBy::Human => "Human",
        };

        serde_json::json!({
            "name": { "title": [{ "text": { "content": &self.name } }] },
            "requirement": { "relation": [{ "id": &self.requirement_id }] },
            "type": { "select": { "name": test_type_str } },
            "priority": { "select": { "name": priority_str } },
            "status": { "select": { "name": status_str } },
            "yaml_script": { "code": { "language": "yaml", "caption": [], "content": &self.yaml_script } },
            "expected_result": { "rich_text": [{ "text": { "content": &self.expected_result } }] },
            "tags": { "multi_select": self.tags.iter().map(|t| serde_json::json!({ "name": t })).collect::<Vec<_>>() },
            "created_by": { "select": { "name": created_by_str } },
            "reviewed_by": self.reviewed_by.as_ref().map(|r| serde_json::json!({ "select": { "name": r } })).unwrap_or(serde_json::json!({ "select": null }))
        })
    }
}
