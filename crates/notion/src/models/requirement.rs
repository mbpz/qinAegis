use serde::{Deserialize, Serialize};
use crate::models::Priority;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub description: String,
    pub priority: Priority,
    pub status: RequirementStatus,
    pub test_case_count: u32,
    pub pass_rate: Option<String>,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Draft,
    Active,
    Done,
}

impl Requirement {
    pub fn from_notion_page(page: &serde_json::Value) -> Option<Self> {
        let id = page["id"].as_str()?.to_string();
        let name = page["properties"]["name"]["title"]
            .as_array()?
            .first()?
            .get("text")?
            .get("content")?
            .as_str()?
            .to_string();

        let project_id = page["properties"]["project"]["relation"]
            .as_array()?
            .first()?
            .get("id")?
            .as_str()?
            .to_string();

        let description = page["properties"]["description"]["rich_text"]
            .as_array()?
            .iter()
            .map(|v| v["text"]["content"].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("");

        let priority_str = page["properties"]["priority"]["select"]["name"]
            .as_str()
            .unwrap_or("p1");
        let priority = match priority_str {
            "p0" => Priority::P0,
            "p2" => Priority::P2,
            _ => Priority::P1,
        };

        let status_str = page["properties"]["status"]["select"]["name"]
            .as_str()
            .unwrap_or("draft");
        let status = match status_str {
            "active" => RequirementStatus::Active,
            "done" => RequirementStatus::Done,
            _ => RequirementStatus::Draft,
        };

        let test_case_count = page["properties"]["test_case_count"]["rollup"]["number"]
            .as_u64()
            .unwrap_or(0) as u32;

        let pass_rate = page["properties"]["pass_rate"]["formula"]["string"]
            .as_str()
            .map(String::from);

        let last_run_at = page["properties"]["last_run_at"]["date"]["start"]
            .as_str()
            .map(String::from);

        Some(Requirement {
            id,
            name,
            project_id,
            description,
            priority,
            status,
            test_case_count,
            pass_rate,
            last_run_at,
        })
    }
}
