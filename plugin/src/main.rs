#[cfg(target_arch = "wasm32")]
use zellij_agent_tab_bar::AgentTabBar;
#[cfg(target_arch = "wasm32")]
use zellij_tile::prelude::*;

#[cfg(target_arch = "wasm32")]
register_plugin!(AgentTabBar);

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
