//! Flexbox Layout Algorithm
//!
//! This module implements the CSS Flexible Box Layout (Flexbox) algorithm.
//! Flexbox provides a more efficient way to lay out, align and distribute
//! space among items in a container.
//!
//! ## Flexbox Concepts
//!
//! - **Main Axis**: Primary axis (row = horizontal, column = vertical)
//! - **Cross Axis**: Perpendicular to main axis
//! - **Flex Container**: Parent element with display: flex
//! - **Flex Items**: Direct children of flex container

use alloc::vec::Vec;
use kpio_css::values::{FlexDirection, FlexWrap, JustifyContent, AlignItems, AlignContent};
use crate::box_model::{BoxDimensions, Rect, EdgeSizes, ResolvedLength};
use crate::layout_box::{LayoutBox, BoxType, ContainingBlock, LayoutContext};

/// Flex container properties
#[derive(Debug, Clone)]
pub struct FlexContainerStyle {
    /// Direction of the main axis
    pub flex_direction: FlexDirection,
    /// Whether items can wrap to multiple lines
    pub flex_wrap: FlexWrap,
    /// Alignment along main axis
    pub justify_content: JustifyContent,
    /// Alignment along cross axis
    pub align_items: AlignItems,
    /// Alignment of lines (when wrapping)
    pub align_content: AlignContent,
}

impl Default for FlexContainerStyle {
    fn default() -> Self {
        Self {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Nowrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_content: AlignContent::Stretch, // Default behavior
        }
    }
}

/// Flex item properties
#[derive(Debug, Clone)]
pub struct FlexItemStyle {
    /// Grow factor
    pub flex_grow: f32,
    /// Shrink factor
    pub flex_shrink: f32,
    /// Initial size on main axis
    pub flex_basis: ResolvedLength,
    /// Override align-items for this item
    pub align_self: Option<AlignItems>,
    /// Order for reordering
    pub order: i32,
}

impl Default for FlexItemStyle {
    fn default() -> Self {
        Self {
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: ResolvedLength::Auto,
            align_self: None,
            order: 0,
        }
    }
}

/// A flex line containing items that fit on one line
#[derive(Debug)]
struct FlexLine {
    /// Items on this line (indices into children)
    items: Vec<usize>,
    /// Main size of items on this line
    main_size: f32,
    /// Cross size of this line
    cross_size: f32,
}

impl FlexLine {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            main_size: 0.0,
            cross_size: 0.0,
        }
    }
}

/// Perform flexbox layout on a flex container
pub fn layout_flex(
    layout_box: &mut LayoutBox,
    containing_block: ContainingBlock,
    context: &LayoutContext,
) {
    let container_style = FlexContainerStyle::default(); // TODO: get from style
    
    // Step 1: Determine main and cross axes
    let is_row = matches!(container_style.flex_direction, FlexDirection::Row | FlexDirection::RowReverse);
    let is_reversed = matches!(container_style.flex_direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);
    
    // Container dimensions
    let container_main_size = if is_row { containing_block.width } else { containing_block.height };
    let container_cross_size = if is_row { containing_block.height } else { containing_block.width };
    
    // Step 2: Calculate flex basis for each item
    let mut item_data: Vec<FlexItemData> = layout_box.children.iter()
        .enumerate()
        .filter(|(_, child)| child.box_type != BoxType::None)
        .map(|(i, child)| {
            let style = FlexItemStyle::default(); // TODO: get from child style
            let base_size = calculate_flex_basis(child, &style, is_row, context);
            FlexItemData {
                index: i,
                style,
                base_size,
                main_size: base_size,
                cross_size: 0.0,
                main_position: 0.0,
                cross_position: 0.0,
            }
        })
        .collect();
    
    // Step 3: Collect items into flex lines
    let lines = collect_flex_lines(
        &item_data,
        container_main_size,
        container_style.flex_wrap,
    );
    
    // Step 4: Resolve flexible lengths
    for line in &lines {
        resolve_flexible_lengths(
            &mut item_data,
            line,
            container_main_size,
        );
    }
    
    // Step 5: Calculate cross sizes
    for line_idx in 0..lines.len() {
        calculate_cross_sizes(
            &mut item_data,
            &lines[line_idx],
            container_cross_size,
            &container_style,
            is_row,
        );
    }
    
    // Step 6: Determine line cross sizes
    let mut line_cross_sizes: Vec<f32> = lines.iter()
        .map(|line| {
            line.items.iter()
                .map(|&idx| item_data[idx].cross_size)
                .fold(0.0f32, |a, b| a.max(b))
        })
        .collect();
    
    // Step 7: Align items on cross axis within each line
    // Step 8: Align lines on cross axis
    let total_lines_cross = line_cross_sizes.iter().sum::<f32>();
    let cross_free_space = (container_cross_size - total_lines_cross).max(0.0);
    
    // Step 9: Calculate positions
    let mut current_main;
    let mut current_cross = if is_row { containing_block.y } else { containing_block.x };
    
    for (line_idx, line) in lines.iter().enumerate() {
        let line_cross_size = line_cross_sizes[line_idx];
        
        // Justify content: distribute main axis space
        let items_main_size: f32 = line.items.iter()
            .map(|&idx| item_data[idx].main_size)
            .sum();
        let main_free_space = (container_main_size - items_main_size).max(0.0);
        
        let (start_offset, gap) = calculate_justify_spacing(
            container_style.justify_content,
            main_free_space,
            line.items.len(),
        );
        
        current_main = if is_row { containing_block.x } else { containing_block.y };
        current_main += start_offset;
        
        if is_reversed {
            current_main = if is_row { containing_block.x + container_main_size } else { containing_block.y + container_main_size };
        }
        
        for &item_idx in &line.items {
            let item = &mut item_data[item_idx];
            
            // Set main position
            if is_reversed {
                current_main -= item.main_size;
                item.main_position = current_main;
            } else {
                item.main_position = current_main;
                current_main += item.main_size + gap;
            }
            
            // Set cross position (align-items)
            let align = item.style.align_self.unwrap_or(container_style.align_items);
            item.cross_position = align_item_cross(
                align,
                current_cross,
                line_cross_size,
                item.cross_size,
            );
        }
        
        current_cross += line_cross_size;
    }
    
    // Step 10: Apply positions to layout boxes
    for item in &item_data {
        let child = &mut layout_box.children[item.index];
        
        if is_row {
            child.dimensions.content.x = item.main_position;
            child.dimensions.content.y = item.cross_position;
            child.dimensions.content.width = item.main_size;
            child.dimensions.content.height = item.cross_size;
        } else {
            child.dimensions.content.x = item.cross_position;
            child.dimensions.content.y = item.main_position;
            child.dimensions.content.width = item.cross_size;
            child.dimensions.content.height = item.main_size;
        }
    }
    
    // Set container dimensions
    layout_box.dimensions.content.width = containing_block.width;
    layout_box.dimensions.content.height = if lines.is_empty() {
        0.0
    } else {
        line_cross_sizes.iter().sum()
    };
}

/// Data for a flex item during layout
#[derive(Debug)]
struct FlexItemData {
    index: usize,
    style: FlexItemStyle,
    base_size: f32,
    main_size: f32,
    cross_size: f32,
    main_position: f32,
    cross_position: f32,
}

/// Calculate flex basis for an item
fn calculate_flex_basis(
    layout_box: &LayoutBox,
    style: &FlexItemStyle,
    is_row: bool,
    _context: &LayoutContext,
) -> f32 {
    match style.flex_basis {
        ResolvedLength::Px(px) => px,
        ResolvedLength::Auto => {
            // Use content size or specified width/height
            if is_row {
                layout_box.style.width.to_px().max(50.0) // Minimum size
            } else {
                layout_box.style.height.to_px().max(20.0)
            }
        }
    }
}

/// Collect items into flex lines
fn collect_flex_lines(
    items: &[FlexItemData],
    container_main_size: f32,
    wrap: FlexWrap,
) -> Vec<FlexLine> {
    let mut lines = Vec::new();
    let mut current_line = FlexLine::new();
    let mut current_main_size = 0.0;
    
    for (i, item) in items.iter().enumerate() {
        let item_size = item.base_size;
        
        // Check if we need to wrap
        let would_overflow = current_main_size + item_size > container_main_size;
        let should_wrap = would_overflow && 
            !matches!(wrap, FlexWrap::Nowrap) && 
            !current_line.items.is_empty();
        
        if should_wrap {
            current_line.main_size = current_main_size;
            lines.push(current_line);
            current_line = FlexLine::new();
            current_main_size = 0.0;
        }
        
        current_line.items.push(i);
        current_main_size += item_size;
    }
    
    if !current_line.items.is_empty() {
        current_line.main_size = current_main_size;
        lines.push(current_line);
    }
    
    lines
}

/// Resolve flexible lengths for items on a line
fn resolve_flexible_lengths(
    items: &mut [FlexItemData],
    line: &FlexLine,
    container_main_size: f32,
) {
    let total_base: f32 = line.items.iter()
        .map(|&idx| items[idx].base_size)
        .sum();
    
    let free_space = container_main_size - total_base;
    
    if free_space > 0.0 {
        // Positive free space: grow
        let total_grow: f32 = line.items.iter()
            .map(|&idx| items[idx].style.flex_grow)
            .sum();
        
        if total_grow > 0.0 {
            for &idx in &line.items {
                let grow_ratio = items[idx].style.flex_grow / total_grow;
                items[idx].main_size = items[idx].base_size + free_space * grow_ratio;
            }
        }
    } else if free_space < 0.0 {
        // Negative free space: shrink
        let total_shrink: f32 = line.items.iter()
            .map(|&idx| items[idx].style.flex_shrink * items[idx].base_size)
            .sum();
        
        if total_shrink > 0.0 {
            for &idx in &line.items {
                let shrink_ratio = (items[idx].style.flex_shrink * items[idx].base_size) / total_shrink;
                items[idx].main_size = (items[idx].base_size + free_space * shrink_ratio).max(0.0);
            }
        }
    }
}

/// Calculate cross sizes for items on a line
fn calculate_cross_sizes(
    items: &mut [FlexItemData],
    line: &FlexLine,
    _container_cross_size: f32,
    container_style: &FlexContainerStyle,
    _is_row: bool,
) {
    for &idx in &line.items {
        let align = items[idx].style.align_self.unwrap_or(container_style.align_items);
        
        // For stretch, use container cross size (simplified)
        items[idx].cross_size = match align {
            AlignItems::Stretch => 50.0, // Would use actual content/container
            _ => 30.0, // Would use actual content size
        };
    }
}

/// Calculate spacing for justify-content
fn calculate_justify_spacing(
    justify: JustifyContent,
    free_space: f32,
    item_count: usize,
) -> (f32, f32) {
    if item_count == 0 {
        return (0.0, 0.0);
    }
    
    match justify {
        JustifyContent::FlexStart | JustifyContent::Start => (0.0, 0.0),
        JustifyContent::FlexEnd | JustifyContent::End => (free_space, 0.0),
        JustifyContent::Center => (free_space / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            if item_count == 1 {
                (0.0, 0.0)
            } else {
                (0.0, free_space / (item_count - 1) as f32)
            }
        }
        JustifyContent::SpaceAround => {
            let gap = free_space / item_count as f32;
            (gap / 2.0, gap)
        }
        JustifyContent::SpaceEvenly => {
            let gap = free_space / (item_count + 1) as f32;
            (gap, gap)
        }
    }
}

/// Align an item on the cross axis
fn align_item_cross(
    align: AlignItems,
    line_start: f32,
    line_size: f32,
    item_size: f32,
) -> f32 {
    match align {
        AlignItems::FlexStart | AlignItems::Start => line_start,
        AlignItems::FlexEnd | AlignItems::End => line_start + line_size - item_size,
        AlignItems::Center => line_start + (line_size - item_size) / 2.0,
        AlignItems::Stretch => line_start, // Size was already stretched
        AlignItems::Baseline => line_start, // Would need font metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_justify_spacing() {
        // space-between with 3 items and 30px free space
        let (start, gap) = calculate_justify_spacing(
            JustifyContent::SpaceBetween,
            30.0,
            3,
        );
        assert_eq!(start, 0.0);
        assert_eq!(gap, 15.0); // 30 / 2 = 15
        
        // center with 30px free space
        let (start, gap) = calculate_justify_spacing(
            JustifyContent::Center,
            30.0,
            3,
        );
        assert_eq!(start, 15.0);
        assert_eq!(gap, 0.0);
    }
}
