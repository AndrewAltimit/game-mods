//! Shared framework for AI consultation MCP servers.
//!
//! Provides common types, traits, and generic tool implementations used by
//! the gemini, crush, and opencode MCP servers to eliminate duplication.
//! (Note: codex support is disabled — OpenAI phased out due to security concerns.)

mod tools;
mod types;

pub use tools::{ClearHistoryTool, ConsultTool, StatusTool, ToggleAutoConsultTool, make_tools};
pub use types::{
    AiIntegration, ConsultParams, ConsultResult, ConsultStatus, HistoryEntry, IntegrationStats,
};
