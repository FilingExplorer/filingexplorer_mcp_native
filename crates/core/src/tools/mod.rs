//! Tool implementations and registry for FilingExplorer MCP.
//!
//! This module contains:
//! - Tool registry with metadata and search functionality
//! - Individual tool implementations organized by category

pub mod registry;

// Tool implementation modules (to be added)
// pub mod company;
// pub mod sec_documents;
// pub mod institutional;
// pub mod etf;
// pub mod form_adv;
// pub mod lobbying;
// pub mod watchlists;

pub use registry::{
    get_categories, get_tool_metadata, list_tools_by_category, search_tools,
    Category, DetailLevel, SearchResult, Tool, ToolCategory,
};
