//! CSS Cascade - Style cascading and inheritance

use alloc::vec::Vec;
use alloc::collections::BTreeMap;

use crate::properties::{PropertyId, PropertyDeclaration, DeclarationBlock};
use crate::selector::Specificity;
use crate::stylesheet::StylesheetOrigin;

/// Cascaded values for an element, before computation.
#[derive(Debug, Clone, Default)]
pub struct CascadedValues {
    /// Property declarations ordered by cascade priority
    declarations: BTreeMap<u16, CascadeEntry>,
}

/// An entry in the cascade.
#[derive(Debug, Clone)]
struct CascadeEntry {
    value: PropertyDeclaration,
    priority: CascadePriority,
}

/// Priority for cascade ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CascadePriority {
    /// Origin weight (higher = more important)
    origin: u8,
    /// Whether !important
    important: bool,
    /// Specificity
    specificity: u32,
    /// Source order (later = higher)
    order: u32,
}

impl CascadedValues {
    /// Create a new empty cascaded values.
    pub fn new() -> Self {
        CascadedValues {
            declarations: BTreeMap::new(),
        }
    }

    /// Apply declarations from a declaration block.
    pub fn apply(
        &mut self,
        declarations: &DeclarationBlock,
        specificity: Specificity,
        origin: StylesheetOrigin,
        order: u32,
    ) {
        for decl in declarations.iter() {
            self.apply_declaration(decl.clone(), specificity, origin, order);
        }
    }

    /// Apply a single declaration.
    pub fn apply_declaration(
        &mut self,
        decl: PropertyDeclaration,
        specificity: Specificity,
        origin: StylesheetOrigin,
        order: u32,
    ) {
        let priority = CascadePriority {
            origin: origin.priority(),
            important: decl.important,
            specificity: specificity.to_u32(),
            order,
        };

        let key = decl.property as u16;
        
        // Check if we should override
        let should_insert = match self.declarations.get(&key) {
            Some(existing) => priority > existing.priority,
            None => true,
        };

        if should_insert {
            self.declarations.insert(key, CascadeEntry {
                value: decl,
                priority,
            });
        }
    }

    /// Apply inline styles (highest specificity except !important).
    pub fn apply_inline(&mut self, declarations: &DeclarationBlock, order: u32) {
        self.apply(
            declarations,
            Specificity::INLINE,
            StylesheetOrigin::Author,
            order,
        );
    }

    /// Get the cascaded value for a property.
    pub fn get(&self, property: PropertyId) -> Option<&PropertyDeclaration> {
        self.declarations
            .get(&(property as u16))
            .map(|e| &e.value)
    }

    /// Check if a property has a cascaded value.
    pub fn has(&self, property: PropertyId) -> bool {
        self.declarations.contains_key(&(property as u16))
    }

    /// Iterate over all cascaded declarations.
    pub fn iter(&self) -> impl Iterator<Item = &PropertyDeclaration> {
        self.declarations.values().map(|e| &e.value)
    }

    /// Get the number of cascaded properties.
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

/// Builder for cascading styles.
pub struct CascadeBuilder {
    order_counter: u32,
}

impl CascadeBuilder {
    /// Create a new cascade builder.
    pub fn new() -> Self {
        CascadeBuilder { order_counter: 0 }
    }

    /// Apply a declaration block and increment order.
    pub fn apply(
        &mut self,
        cascaded: &mut CascadedValues,
        declarations: &DeclarationBlock,
        specificity: Specificity,
        origin: StylesheetOrigin,
    ) {
        cascaded.apply(declarations, specificity, origin, self.order_counter);
        self.order_counter += 1;
    }

    /// Apply inline styles.
    pub fn apply_inline(&mut self, cascaded: &mut CascadedValues, declarations: &DeclarationBlock) {
        cascaded.apply_inline(declarations, self.order_counter);
        self.order_counter += 1;
    }
}

impl Default for CascadeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
