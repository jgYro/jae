//! Editor module for JAE - Just Another Editor.
//!
//! This module contains the core Editor struct and all supporting types
//! organized into submodules for maintainability.

// Core modules
mod core;
pub mod buffer_ops;
pub mod dialogs;
pub mod settings;
pub mod syntax;
pub mod text_widget;
pub mod types;
pub mod undo;

// Operation modules
mod file_ops;
mod menu;
mod movement;
mod selection;

// Re-export the Editor struct and core types
pub use core::{Editor, RecenterState};

// Re-export dialog types
pub use dialogs::{ConfirmationDialog, DeleteFileConfirmation, QuitConfirmation};

// Re-export settings
pub use settings::Settings;

// Re-export commonly used types
pub use types::{
    CommandInfo, FloatingMode, FloatingWindow, JumpMode, JumpPhase, JumpTarget, MarkState,
    MenuAction, MenuItem, MenuState, MinibufferCallback, ResponseResult, ResponseType,
    SettingItem, SettingValue, StatusBarState,
};

// Re-export undo types
pub use undo::UndoManager;
