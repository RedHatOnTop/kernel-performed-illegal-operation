//! KPIO Layout Engine
//!
//! This crate implements CSS layout algorithms for the KPIO browser engine.
//! It takes a styled DOM tree and produces a layout tree with computed positions
//! and dimensions for each box.
//!
//! ## Layout Pipeline
//!
//! ```text
//! StyledNode (from kpio-dom/kpio-css)
//!     ↓
//! LayoutBox (with BoxType: Block, Inline, Anonymous)
//!     ↓
//! Layout Algorithm (block, inline, flex)
//!     ↓
//! BoxDimensions (position, size, margins, etc.)
//!     ↓
//! DisplayList (paint commands for renderer)
//! ```

#![no_std]

extern crate alloc;

pub mod block;
pub mod box_model;
pub mod flex;
pub mod inline;
pub mod layout_box;
pub mod paint;
pub mod parallel;

pub use box_model::{BoxDimensions, EdgeSizes, Rect};
pub use layout_box::{BoxType, LayoutBox};
pub use paint::{DisplayCommand, DisplayList};
pub use parallel::{LayoutTask, ParallelLayoutContext, ParallelScheduler};

/// Prelude for common imports
pub mod prelude {
    pub use crate::{BoxDimensions, EdgeSizes, Rect};
    pub use crate::{BoxType, LayoutBox};
    pub use crate::{DisplayCommand, DisplayList};
}
