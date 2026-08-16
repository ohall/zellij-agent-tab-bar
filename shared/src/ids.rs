use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self(u64::from(value))
            }
        }

        impl From<usize> for $name {
            fn from(value: usize) -> Self {
                Self(u64::try_from(value).unwrap_or(u64::MAX))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

numeric_id!(TabId);
numeric_id!(PaneId);
numeric_id!(Generation);
numeric_id!(Sequence);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub const DEFAULT_VALUE: &'static str = "default";

    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self(Self::DEFAULT_VALUE.to_owned())
    }
}

impl From<String> for AgentId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for AgentId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
