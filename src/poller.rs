use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollTarget {
    pub target_name: String,
    pub command_path: PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_args: Vec<String>,
}
