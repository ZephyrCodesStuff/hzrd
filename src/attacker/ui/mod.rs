// Export all UI submodules
mod input;
mod logging;
mod rendering;
mod state;
mod status;
mod tabs;

// The main UI entry point
pub use state::run_ui;
