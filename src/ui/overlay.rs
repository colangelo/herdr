//! The overlay kit: the geometry, rows, and cursors every panel, modal, and
//! picker is built from.
//!
//! Ratatui deliberately ships no overlay or component model, so a ratatui app
//! grows its own layer there. Herdr's grew by copy-adapt until two panels were
//! running byte-for-byte parallel arithmetic. This module is that arithmetic,
//! written once.
//!
//! Nothing here holds mutable widget state: `compute_view` mutates, `render`
//! does not, and the kit is data and geometry rather than a widget tree.

mod geometry;

pub(crate) use geometry::{AnchoredPanelSpec, PanelGeometry, VerticalAnchor};
