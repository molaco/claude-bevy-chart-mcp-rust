//! Panes module - Multi-pane layout management for chart rendering.
//!
//! This module provides the pane system for organizing chart content
//! into separate areas (price, volume, indicators) with automatic
//! layout calculation and coordinate space management.
//!
//! # Contents
//!
//! - [`Pane`]: Individual pane configuration
//! - [`PaneId`]: Pane identifier enum
//! - [`PaneType`]: Content type for each pane
//! - [`PaneManager`]: Bevy resource for managing multi-pane layout
//! - [`CrosshairEntities`]: Persistent crosshair entity references
//! - [`GridBorderEntities`]: Persistent grid and border entity references

mod pane;
mod manager;
mod entities;

pub use pane::{Pane, PaneId, PaneType};
pub use manager::PaneManager;
pub use entities::{CrosshairEntities, GridBorderEntities};
