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
#![allow(dead_code)]

extern crate alloc;

pub mod box_model;
pub mod layout_box;
pub mod block;
pub mod inline;
pub mod flex;
pub mod paint;
pub mod parallel;

pub use box_model::{Rect, EdgeSizes, BoxDimensions};
pub use layout_box::{LayoutBox, BoxType};
pub use paint::{DisplayList, DisplayCommand};
pub use parallel::{ParallelScheduler, ParallelLayoutContext, LayoutTask};

/// Prelude for common imports
pub mod prelude {
    pub use crate::{Rect, EdgeSizes, BoxDimensions};
    pub use crate::{LayoutBox, BoxType};
    pub use crate::{DisplayList, DisplayCommand};
}
