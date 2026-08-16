#![doc = "Shared, transport-safe models for the Agent-Aware Zellij Tab Bar."]
#![forbid(unsafe_code)]

pub mod config;
pub mod directory;
pub mod event;
pub mod ids;
pub mod render;
pub mod state;
pub mod status;
pub mod traits;

pub use config::{
    AnimationConfig, BadgeConfig, BehaviorConfig, Config, ConfigError, DebugConfig, LayoutConfig,
    ThemeConfig,
};
pub use directory::{basename, resolve_tab_names, DirectoryPath, NameSource, ResolvedTabName};
pub use event::{Event, EventKind, EVENT_SCHEMA_VERSION};
pub use ids::{AgentId, Generation, PaneId, Sequence, TabId};
pub use render::{
    render_tab_bar, sanitize_terminal_text, truncate_to_width, RenderModel, RenderOutput, RenderTab,
};
pub use state::{AgentRecord, ApplyOutcome, PaneState, Reduction, State, StateError, TabState};
pub use status::AgentStatus;
pub use traits::{DirectoryResolver, Renderer, StatusStore, Transport};

#[cfg(test)]
mod tests;
