//! Symbol Resolution
//!
//! Resolves addresses to symbol names for readable backtraces.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::StackFrame;

/// Symbol table
pub struct SymbolTable {
    /// Symbols sorted by address
    symbols: Vec<Symbol>,
    /// Name to address lookup
    name_map: BTreeMap<String, usize>,
    /// Loaded
    loaded: bool,
}

impl SymbolTable {
    /// Create new symbol table
    pub const fn new() -> Self {
        Self {
            symbols: Vec::new(),
            name_map: BTreeMap::new(),
            loaded: false,
        }
    }

    /// Load symbols from kernel binary
    pub fn load(&mut self, _kernel_base: u64) {
        // Would parse ELF symbol table or .debug_info
        // For now, add some placeholder kernel symbols
        
        self.add_symbol(Symbol {
            name: "_start".to_string(),
            address: 0xFFFF_8000_0000_0000,
            size: 0x100,
            symbol_type: SymbolType::Function,
        });

        self.add_symbol(Symbol {
            name: "kernel_main".to_string(),
            address: 0xFFFF_8000_0000_1000,
            size: 0x500,
            symbol_type: SymbolType::Function,
        });

        self.add_symbol(Symbol {
            name: "page_fault_handler".to_string(),
            address: 0xFFFF_8000_0001_0000,
            size: 0x200,
            symbol_type: SymbolType::Function,
        });

        self.loaded = true;
    }

    /// Add symbol
    pub fn add_symbol(&mut self, symbol: Symbol) {
        let idx = self.symbols.len();
        self.name_map.insert(symbol.name.clone(), idx);
        self.symbols.push(symbol);
        
        // Keep sorted by address
        self.symbols.sort_by_key(|s| s.address);
    }

    /// Find symbol by address
    pub fn find(&self, address: u64) -> Option<(&Symbol, u64)> {
        // Binary search for containing symbol
        let idx = self.symbols
            .binary_search_by(|s| {
                if address < s.address {
                    core::cmp::Ordering::Greater
                } else if address >= s.address + s.size as u64 {
                    core::cmp::Ordering::Less
                } else {
                    core::cmp::Ordering::Equal
                }
            })
            .ok()?;

        let symbol = &self.symbols[idx];
        let offset = address - symbol.address;
        Some((symbol, offset))
    }

    /// Get symbol by name
    pub fn get_by_name(&self, name: &str) -> Option<&Symbol> {
        self.name_map.get(name).map(|&idx| &self.symbols[idx])
    }

    /// Resolve stack frame
    pub fn resolve_frame(&self, frame: &mut StackFrame) {
        if let Some((symbol, offset)) = self.find(frame.address) {
            frame.symbol = Some(symbol.name.clone());
            frame.offset = offset;
        }
    }

    /// Resolve all frames
    pub fn resolve_frames(&self, frames: &mut [StackFrame]) {
        for frame in frames {
            self.resolve_frame(frame);
        }
    }

    /// Is loaded
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Symbol count
    pub fn count(&self) -> usize {
        self.symbols.len()
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Symbol
#[derive(Debug, Clone)]
pub struct Symbol {
    /// Name
    pub name: String,
    /// Address
    pub address: u64,
    /// Size
    pub size: usize,
    /// Type
    pub symbol_type: SymbolType,
}

/// Symbol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    /// Function
    Function,
    /// Object/data
    Object,
    /// Section
    Section,
    /// File
    File,
    /// Unknown
    Unknown,
}

/// Name demangler
pub struct Demangler;

impl Demangler {
    /// Demangle a symbol name
    pub fn demangle(name: &str) -> String {
        // Handle Rust mangled names
        if name.starts_with("_ZN") || name.starts_with("_R") {
            Self::demangle_rust(name)
        } else if name.starts_with("_Z") {
            Self::demangle_cpp(name)
        } else {
            name.to_string()
        }
    }

    /// Demangle Rust symbol
    fn demangle_rust(name: &str) -> String {
        // Simplified Rust demangling
        // Full implementation would handle v0 and legacy mangling

        if name.starts_with("_R") {
            // Rust v0 mangling
            // Would parse properly
            return name.to_string();
        }

        // Legacy Rust mangling (_ZN...)
        let mut result = String::new();
        let chars: Vec<char> = name.chars().collect();
        let mut i = 3; // Skip _ZN

        while i < chars.len() {
            // Read length
            let mut len_str = String::new();
            while i < chars.len() && chars[i].is_ascii_digit() {
                len_str.push(chars[i]);
                i += 1;
            }

            if len_str.is_empty() {
                break;
            }

            let len: usize = len_str.parse().unwrap_or(0);
            if len == 0 {
                break;
            }

            // Add separator
            if !result.is_empty() {
                result.push_str("::");
            }

            // Read name
            for _ in 0..len {
                if i < chars.len() {
                    result.push(chars[i]);
                    i += 1;
                }
            }
        }

        if result.is_empty() {
            name.to_string()
        } else {
            result
        }
    }

    /// Demangle C++ symbol (simplified)
    fn demangle_cpp(name: &str) -> String {
        // Would implement Itanium C++ ABI demangling
        // For now, just return as-is
        name.to_string()
    }
}

/// Global symbol table
use spin::Mutex;
pub static SYMBOL_TABLE: Mutex<SymbolTable> = Mutex::new(SymbolTable::new());

/// Initialize symbol table
pub fn init(kernel_base: u64) {
    let mut table = SYMBOL_TABLE.lock();
    table.load(kernel_base);
}

/// Resolve address to symbol
pub fn resolve(address: u64) -> Option<(String, u64)> {
    let table = SYMBOL_TABLE.lock();
    table.find(address).map(|(s, o)| (s.name.clone(), o))
}
