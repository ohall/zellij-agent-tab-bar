use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    #[default]
    Idle,
    Running,
    Complete,
    Error,
}

impl AgentStatus {
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Complete => 1,
            Self::Error => 2,
            Self::Running => 3,
        }
    }

    #[must_use]
    pub fn aggregate(statuses: impl IntoIterator<Item = Self>) -> Self {
        statuses
            .into_iter()
            .max_by_key(|status| status.priority())
            .unwrap_or_default()
    }
}
