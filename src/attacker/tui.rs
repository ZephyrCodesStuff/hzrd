// ui.rs - Main entry point for the UI module
// This file now delegates to the modular UI components

/// Run the attacker UI
pub async fn ui(args: crate::cli::Args, config: &crate::structs::config::Config) {
    // Delegate to the modular implementation
    super::ui::run_ui(args, config).await;
}
