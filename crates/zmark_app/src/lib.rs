/// Single source of truth for the version shown in the About tab and bundle.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod actions;
pub mod assets;
pub mod editor_engine;
pub mod local_images;
pub mod platform;
pub mod state;
pub mod syntax;
pub mod theme;
pub mod updater;
pub mod views;
