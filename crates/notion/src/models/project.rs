use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub url: String,
    pub tech_stack: Vec<String>,
    pub status: ProjectStatus,
    pub spec_page_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Archived,
}

impl Project {
    pub fn from_notion_page(page: &serde_json::Value) -> Option<Self> {
        let id = page["id"].as_str()?.to_string();
        let name = page["properties"]["name"]["title"]
            .as_array()?
            .first()?
            .get("text")?
            .get("content")?
            .as_str()?
            .to_string();
        let url = page["properties"]["url"]["url"].as_str()?.to_string();

        let tech_stack = page["properties"]["tech_stack"]["multi_select"]
            .as_array()?
            .iter()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();

        let status_str = page["properties"]["status"]["select"]["name"]
            .as_str()
            .unwrap_or("active");
        let status = match status_str {
            "archived" => ProjectStatus::Archived,
            _ => ProjectStatus::Active,
        };

        let spec_page_id = page["properties"]["spec_page"]["relation"]
            .as_array()?
            .first()?
            .get("id")?
            .as_str()
            .map(String::from);

        let created_at = page["properties"]["created_at"]["date"]["start"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        Some(Project {
            id,
            name,
            url,
            tech_stack,
            status,
            spec_page_id,
            created_at,
        })
    }
}
