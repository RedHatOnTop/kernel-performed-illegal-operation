# Contributing to KPIO

Thank you for your interest in contributing to the KPIO operating system project. This document provides guidelines and information for contributors.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Code Style Guidelines](#code-style-guidelines)
5. [Commit Guidelines](#commit-guidelines)
6. [Pull Request Process](#pull-request-process)
7. [Testing Requirements](#testing-requirements)
8. [Documentation Standards](#documentation-standards)
9. [Issue Reporting](#issue-reporting)
10. [Security Vulnerabilities](#security-vulnerabilities)

---

## Code of Conduct

### Our Standards

- Be respectful and inclusive
- Focus on constructive feedback
- Accept responsibility for mistakes
- Prioritize the project's best interests

### Unacceptable Behavior

- Harassment or discrimination
- Personal attacks
- Publishing private information
- Trolling or inflammatory comments

---

## Getting Started

### Finding Work

1. Check the issue tracker for `good-first-issue` labels
2. Review the roadmap for planned features
3. Look for `help-wanted` issues
4. Propose new features via issue discussion

### Types of Contributions

| Type | Description |
|------|-------------|
| Code | Bug fixes, features, optimizations |
| Documentation | User guides, API docs, comments |
| Testing | Test cases, fuzzing, benchmarks |
| Review | Code review, design feedback |
| Triage | Issue categorization, reproduction |

---

## Development Setup

### Prerequisites

```
Required:
- Rust nightly (see rust-toolchain.toml)
- QEMU 7.0+ for testing
- Python 3.8+ for build scripts

Recommended:
- VS Code with rust-analyzer
- GDB for debugging
- KVM for hardware acceleration
```

### Initial Setup

```powershell
# Clone the repository
git clone https://github.com/kpio/kpio.git
cd kpio

# Install Rust toolchain
rustup show  # Automatically installs from rust-toolchain.toml

# Add required targets
rustup target add x86_64-unknown-none

# Install development tools
cargo install cargo-xbuild
cargo install bootimage

# Verify setup
cargo build --target x86_64-unknown-none

# Run in QEMU
cargo run
```

### IDE Configuration

#### VS Code

Recommended extensions:
- rust-analyzer
- CodeLLDB
- Even Better TOML
- Error Lens

Workspace settings (`.vscode/settings.json`):
```json
{
    "rust-analyzer.cargo.target": "x86_64-unknown-none",
    "rust-analyzer.checkOnSave.allTargets": false,
    "rust-analyzer.cargo.features": ["qemu"]
}
```

---

## Code Style Guidelines

### Rust Style

Follow the official Rust style guide with these additions:

#### Naming

| Item | Convention | Example |
|------|------------|---------|
| Types | PascalCase | `PageTable`, `VfsError` |
| Functions | snake_case | `allocate_frame`, `read_sector` |
| Constants | SCREAMING_SNAKE_CASE | `PAGE_SIZE`, `MAX_CPUS` |
| Modules | snake_case | `memory_manager`, `block_device` |

#### Formatting

```rust
// Good: Clear structure
pub fn allocate_pages(count: usize, flags: PageFlags) -> Result<PhysAddr, MemoryError> {
    if count == 0 {
        return Err(MemoryError::InvalidCount);
    }
    
    let frames = self.frame_allocator
        .allocate_contiguous(count)
        .ok_or(MemoryError::OutOfMemory)?;
    
    Ok(frames)
}

// Bad: Cramped, unclear
pub fn allocate_pages(count: usize, flags: PageFlags) -> Result<PhysAddr, MemoryError> {
    if count == 0 { return Err(MemoryError::InvalidCount); }
    let frames = self.frame_allocator.allocate_contiguous(count).ok_or(MemoryError::OutOfMemory)?;
    Ok(frames)
}
```

#### Comments

```rust
/// Allocates contiguous physical pages.
///
/// # Arguments
///
/// * `count` - Number of pages to allocate
/// * `flags` - Page flags (read/write/execute permissions)
///
/// # Returns
///
/// Physical address of the first page, or an error if allocation fails.
///
/// # Errors
///
/// * `MemoryError::InvalidCount` - If count is zero
/// * `MemoryError::OutOfMemory` - If insufficient memory available
pub fn allocate_pages(count: usize, flags: PageFlags) -> Result<PhysAddr, MemoryError> {
    // Implementation
}
```

### Safety Requirements

For `unsafe` code:

```rust
// SAFETY: Document why this is safe
//
// 1. `ptr` is guaranteed to be valid because [reason]
// 2. The memory is properly aligned for type T
// 3. No other references exist (we hold exclusive lock)
unsafe {
    ptr.write(value);
}
```

### Error Handling

```rust
// Preferred: Use Result with descriptive errors
pub fn read_file(path: &Path) -> Result<Vec<u8>, FileError> {
    let file = File::open(path).map_err(FileError::Open)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).map_err(FileError::Read)?;
    Ok(contents)
}

// Avoid: Panicking on recoverable errors
pub fn read_file(path: &Path) -> Vec<u8> {
    let file = File::open(path).expect("failed to open"); // Don't do this
    // ...
}
```

---

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

[optional body]

[optional footer]
```

### Types

| Type | Description |
|------|-------------|
| feat | New feature |
| fix | Bug fix |
| docs | Documentation only |
| style | Formatting, no code change |
| refactor | Code restructuring |
| perf | Performance improvement |
| test | Adding tests |
| chore | Build/tooling changes |

### Examples

```
feat(memory): implement buddy allocator

Adds a buddy allocator for physical memory management.
Supports allocation sizes from 4KB to 2MB.

Closes #42

---

fix(scheduler): prevent deadlock in task switch

The scheduler could deadlock when switching from an interrupt
context if the run queue lock was already held. This adds
a check for the interrupt flag before acquiring the lock.

---

docs(architecture): add graphics subsystem specification

Comprehensive documentation for the Vulkan-exclusive graphics
stack including DRM/KMS design and compositor architecture.
```

### Commit Best Practices

- Keep commits atomic (one logical change per commit)
- Write in imperative mood ("add feature" not "added feature")
- Reference issues when applicable
- Separate refactoring from functional changes

---

## Pull Request Process

### Before Submitting

1. Create a feature branch from `main`
2. Ensure all tests pass locally
3. Run `cargo clippy` with no warnings
4. Run `cargo fmt` to format code
5. Update documentation if needed
6. Write/update tests for changes

### PR Template

```markdown
## Description

Brief description of changes.

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing

Describe testing performed.

## Checklist

- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] No new warnings
```

### Review Process

1. Create PR with descriptive title
2. Fill out PR template completely
3. Wait for CI to pass
4. Address reviewer feedback
5. Squash/rebase as requested
6. Maintainer merges when approved

### Merge Requirements

- At least one maintainer approval
- All CI checks passing
- No unresolved conversations
- Up-to-date with target branch

---

## Testing Requirements

### Test Categories

| Category | Requirement | Command |
|----------|-------------|---------|
| Unit | Required for all new code | `cargo test` |
| Integration | Required for subsystem changes | `cargo test --test integration` |
| Kernel | Boot and functionality tests | `cargo run --release` |

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn buddy_allocator_basic() {
        let mut allocator = BuddyAllocator::new(1024 * 1024);
        
        // Test single allocation
        let block = allocator.allocate(4096).expect("allocation failed");
        assert!(block.is_aligned(4096));
        
        // Test free
        allocator.free(block);
        
        // Verify memory is reusable
        let block2 = allocator.allocate(4096).expect("reallocation failed");
        assert_eq!(block, block2);
    }
    
    #[test]
    fn buddy_allocator_exhaustion() {
        let mut allocator = BuddyAllocator::new(4096);
        
        let _ = allocator.allocate(4096).expect("first allocation");
        
        // Second allocation should fail
        assert!(allocator.allocate(4096).is_none());
    }
}
```

### Test Coverage

Aim for:
- 80%+ line coverage for new code
- 100% coverage for critical paths (memory, security)
- All error paths tested

---

## Documentation Standards

### Code Documentation

All public items must have documentation:

```rust
/// A virtual memory region descriptor.
///
/// Represents a contiguous range of virtual addresses with associated
/// permissions and backing storage information.
pub struct VmRegion {
    /// Starting virtual address (page-aligned)
    pub start: VirtAddr,
    
    /// Region size in bytes (multiple of page size)
    pub size: usize,
    
    /// Access permissions
    pub permissions: PageFlags,
    
    /// Backing storage type
    pub backing: BackingType,
}
```

### Architecture Documentation

Located in `docs/architecture/`:

- Use Markdown format
- Include ASCII diagrams for visual concepts
- Provide code examples where applicable
- Cross-reference related documents
- Keep updated with implementation changes

### User Documentation

Located in `docs/user/`:

- Clear, step-by-step instructions
- Screenshots/diagrams where helpful
- Troubleshooting sections
- Version-specific information noted

---

## Issue Reporting

### Bug Reports

Include:
1. KPIO version/commit
2. Hardware/VM configuration
3. Steps to reproduce
4. Expected vs. actual behavior
5. Relevant logs/output
6. Possible fix (if known)

### Feature Requests

Include:
1. Use case description
2. Proposed solution
3. Alternative approaches considered
4. Willingness to implement

### Issue Labels

| Label | Meaning |
|-------|---------|
| bug | Something isn't working |
| enhancement | New feature request |
| documentation | Documentation improvement |
| good-first-issue | Suitable for newcomers |
| help-wanted | Extra attention needed |
| priority-high | Critical issue |
| wontfix | Will not be addressed |

---

## Security Vulnerabilities

### Reporting

DO NOT open public issues for security vulnerabilities.

Instead, email: security@kpio.dev

Include:
1. Description of vulnerability
2. Steps to reproduce
3. Potential impact assessment
4. Suggested fix (if any)

### Response Timeline

| Stage | Timeframe |
|-------|-----------|
| Acknowledgment | 48 hours |
| Initial assessment | 1 week |
| Fix development | Varies |
| Public disclosure | After fix released |

### Recognition

Security researchers will be credited in:
- Security advisory
- Release notes
- Hall of fame (with permission)

---

## License

By contributing to KPIO, you agree that your contributions will be licensed under the project's MIT license.

---

## Questions?

- Open a discussion on GitHub
- Join our community chat
- Email: contributors@kpio.dev
