pub mod room;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentInfo {
    pub first_name: String,
    pub last_name: String,
    pub matriculation_number: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", content = "data")]
pub enum ScanResponse {
    #[serde(rename = "success")]
    Success(StudentInfo),
    #[serde(rename = "partial")]
    Partial {
        info: PartialStudentInfo,
        missing: Vec<String>,
    },
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialStudentInfo {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub matriculation_number: Option<String>,
}
