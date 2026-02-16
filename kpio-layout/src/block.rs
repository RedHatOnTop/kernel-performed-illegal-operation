//! Block Layout Algorithm
//!
//! This module implements the CSS block formatting context layout algorithm.
//! Block boxes stack vertically and their width is determined by the containing block.
//!
//! ## Block Layout Rules
//!
//! 1. Width: `margin-left + border-left + padding-left + width + padding-right + border-right + margin-right = containing block width`
//! 2. Height: Sum of children heights (or explicit height if set)
//! 3. Position: Boxes stack vertically, margins collapse

use crate::box_model::{BoxDimensions, EdgeSizes, Rect, ResolvedLength};
use crate::layout_box::{BoxType, ContainingBlock, LayoutBox, LayoutContext};
use alloc::vec::Vec;

/// Perform block layout on a layout box
pub fn layout_block(
    layout_box: &mut LayoutBox,
    containing_block: ContainingBlock,
    context: &LayoutContext,
) {
    // Step 1: Calculate width
    calculate_block_width(layout_box, containing_block);

    // Step 2: Calculate position within containing block
    calculate_block_position(layout_box, containing_block);

    // Step 3: Layout children and calculate height
    layout_block_children(layout_box, context);

    // Step 4: Calculate height (may depend on children)
    calculate_block_height(layout_box);
}

/// Calculate the width of a block element
fn calculate_block_width(layout_box: &mut LayoutBox, containing_block: ContainingBlock) {
    let style = &layout_box.style;
    let d = &mut layout_box.dimensions;

    // Containing block width is our reference
    let container_width = containing_block.width;

    // Get the specified values
    let width = style.width;
    let margin_left = style.margin_left;
    let margin_right = style.margin_right;

    // Set padding and border (these are never auto)
    d.padding = style.padding();
    d.border = style.border();

    // Calculate total of non-auto values
    let padding_border = d.padding.horizontal() + d.border.horizontal();

    // Determine auto values
    let (resolved_width, resolved_margin_left, resolved_margin_right) = resolve_block_width(
        width,
        margin_left,
        margin_right,
        container_width,
        padding_border,
    );

    // Set the computed values
    d.content.width = resolved_width;
    d.margin.left = resolved_margin_left;
    d.margin.right = resolved_margin_right;
}

/// Resolve width and margins for block boxes
///
/// CSS 2.1 Section 10.3.3:
/// If 'width' is not 'auto' and 'border-left-width' + 'padding-left' + 'width' +
/// 'padding-right' + 'border-right-width' (plus any of 'margin-left' or 'margin-right'
/// that are not 'auto') is larger than the width of the containing block, then any
/// 'auto' values for 'margin-left' or 'margin-right' are, for the following rules,
/// treated as zero.
fn resolve_block_width(
    width: ResolvedLength,
    margin_left: ResolvedLength,
    margin_right: ResolvedLength,
    container_width: f32,
    padding_border: f32,
) -> (f32, f32, f32) {
    // Count auto values
    let width_auto = width.is_auto();
    let ml_auto = margin_left.is_auto();
    let mr_auto = margin_right.is_auto();

    // Get non-auto values
    let width_px = width.to_px();
    let ml_px = margin_left.to_px();
    let mr_px = margin_right.to_px();

    // Total of non-auto values
    let total = padding_border
        + if width_auto { 0.0 } else { width_px }
        + if ml_auto { 0.0 } else { ml_px }
        + if mr_auto { 0.0 } else { mr_px };

    // Underflow (positive means we have space left)
    let underflow = container_width - total;

    match (width_auto, ml_auto, mr_auto) {
        // If all three are auto, or if width is auto
        (true, _, _) => {
            // Width fills remaining space, margins are 0 if auto
            let ml = if ml_auto { 0.0 } else { ml_px };
            let mr = if mr_auto { 0.0 } else { mr_px };
            let w = container_width - padding_border - ml - mr;
            (w.max(0.0), ml, mr)
        }

        // Width is not auto, both margins are auto
        (false, true, true) => {
            // Split remaining space equally between margins
            if underflow >= 0.0 {
                let half_underflow = underflow / 2.0;
                (width_px, half_underflow, half_underflow)
            } else {
                // Overconstrained: right margin becomes negative
                (width_px, 0.0, underflow)
            }
        }

        // Width is not auto, only left margin is auto
        (false, true, false) => {
            let ml = underflow.max(0.0);
            (width_px, ml, mr_px)
        }

        // Width is not auto, only right margin is auto
        (false, false, true) => {
            let mr = underflow.max(0.0);
            (width_px, ml_px, mr)
        }

        // Nothing is auto (overconstrained)
        (false, false, false) => {
            // Adjust right margin to account for overflow/underflow
            (width_px, ml_px, mr_px + underflow)
        }
    }
}

/// Calculate the position of a block element within its containing block
fn calculate_block_position(layout_box: &mut LayoutBox, containing_block: ContainingBlock) {
    let d = &mut layout_box.dimensions;

    // Margins are already set (top/bottom default to 0 for now)
    d.margin.top = layout_box.style.margin_top.to_px();
    d.margin.bottom = layout_box.style.margin_bottom.to_px();

    // Position content box
    // X position: containing block x + margin + border + padding
    d.content.x = containing_block.x + d.margin.left + d.border.left + d.padding.left;

    // Y position will be set by the parent during child layout
    // For now, set it relative to containing block top
    d.content.y = containing_block.y + d.margin.top + d.border.top + d.padding.top;
}

/// Layout children of a block element
fn layout_block_children(layout_box: &mut LayoutBox, context: &LayoutContext) {
    let d = &layout_box.dimensions;

    // Create containing block for children
    let child_containing_block = ContainingBlock {
        width: d.content.width,
        height: 0.0, // Height is auto for now
        x: d.content.x,
        y: d.content.y,
    };

    // Track current Y position
    let mut current_y = d.content.y;

    // Previous margin for collapsing
    let mut prev_margin_bottom = 0.0f32;

    for child in &mut layout_box.children {
        match child.box_type {
            BoxType::Block | BoxType::AnonymousBlock => {
                // Create containing block at current Y
                let cb = ContainingBlock {
                    width: child_containing_block.width,
                    height: child_containing_block.height,
                    x: child_containing_block.x,
                    y: current_y,
                };

                // Margin collapsing
                let child_margin_top = child.style.margin_top.to_px();
                let collapsed_margin = prev_margin_bottom.max(child_margin_top);

                // Adjust Y for margin collapsing (only collapse, not add)
                if prev_margin_bottom > 0.0 {
                    // Remove the previous margin, use collapsed instead
                    let adjustment = prev_margin_bottom - collapsed_margin;
                    // current_y -= adjustment; // Would apply margin collapsing
                }

                // Layout the child
                layout_block(child, cb, context);

                // Move Y down past this child
                current_y = child.dimensions.margin_box().bottom();
                prev_margin_bottom = child.style.margin_bottom.to_px();
            }
            BoxType::Inline | BoxType::AnonymousInline => {
                // Inline boxes are handled by inline layout
                // For now, just give them some height
                child.dimensions.content.x = child_containing_block.x;
                child.dimensions.content.y = current_y;
                child.dimensions.content.width = child_containing_block.width;
                child.dimensions.content.height = 20.0; // Line height approximation
                current_y += 20.0;
            }
            BoxType::None => {
                // Skip display: none
            }
        }
    }
}

/// Calculate the height of a block element
fn calculate_block_height(layout_box: &mut LayoutBox) {
    // If explicit height is set, use it
    if let ResolvedLength::Px(h) = layout_box.style.height {
        layout_box.dimensions.content.height = h;
        return;
    }

    // Otherwise, height is sum of children
    let height = if layout_box.children.is_empty() {
        0.0
    } else {
        // Height is from content top to last child's margin box bottom
        let content_y = layout_box.dimensions.content.y;
        let last_child_bottom = layout_box
            .children
            .last()
            .map(|c| c.dimensions.margin_box().bottom())
            .unwrap_or(content_y);

        (last_child_bottom - content_y).max(0.0)
    };

    layout_box.dimensions.content.height = height;
}

/// Build layout tree from styled DOM and perform layout
pub fn build_layout_tree(
    root_style: &kpio_css::computed::ComputedStyle,
    root_node_id: kpio_dom::NodeId,
    viewport_width: f32,
    viewport_height: f32,
) -> LayoutBox {
    let context = LayoutContext::new(viewport_width, viewport_height);
    let containing_block = ContainingBlock::new(viewport_width, viewport_height);

    let mut root_box = LayoutBox::from_style(root_style, root_node_id);
    layout_block(&mut root_box, containing_block, &context);

    root_box
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_width_resolution() {
        // Container 800px, width auto, margins 0
        let (w, ml, mr) = resolve_block_width(
            ResolvedLength::Auto,
            ResolvedLength::Px(0.0),
            ResolvedLength::Px(0.0),
            800.0,
            0.0,
        );
        assert_eq!(w, 800.0);
        assert_eq!(ml, 0.0);
        assert_eq!(mr, 0.0);

        // Container 800px, width 400px, margins auto (centering)
        let (w, ml, mr) = resolve_block_width(
            ResolvedLength::Px(400.0),
            ResolvedLength::Auto,
            ResolvedLength::Auto,
            800.0,
            0.0,
        );
        assert_eq!(w, 400.0);
        assert_eq!(ml, 200.0);
        assert_eq!(mr, 200.0);
    }
}
