use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    pub name: String,
    pub property_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSpec {
    pub name: String,
    pub properties: Vec<PropertySchema>,
}