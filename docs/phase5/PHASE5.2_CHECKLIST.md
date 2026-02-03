# Sub-Phase 5.2: Comprehensive Unit Testing Checklist

## Overview
Create comprehensive unit test coverage for all modules across kernel, browser, and apps.

## Test Coverage Targets

| Component | Current | Target | Priority |
|-----------|---------|--------|----------|
| Kernel Memory | 0% | 90% | Critical |
| Kernel Process | 0% | 85% | Critical |
| Kernel Syscall | 0% | 95% | Critical |
| Browser Core | 0% | 80% | High |
| Browser Apps | 0% | 70% | Medium |
| HTML/CSS Parsers | 0% | 85% | High |

---

## 5.2.1 Kernel Unit Tests

### Memory Management Tests
**File**: `kernel/src/tests/memory_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| M001 | `test_buddy_alloc_single` | Allocate single page | ⬜ |
| M002 | `test_buddy_alloc_multi` | Allocate multiple pages | ⬜ |
| M003 | `test_buddy_free` | Free allocated pages | ⬜ |
| M004 | `test_buddy_coalesce` | Verify buddy coalescing | ⬜ |
| M005 | `test_buddy_fragmentation` | Alloc/free pattern | ⬜ |
| M006 | `test_slab_alloc` | Slab allocator basic | ⬜ |
| M007 | `test_slab_cache_create` | Create slab cache | ⬜ |
| M008 | `test_slab_reuse` | Verify object reuse | ⬜ |
| M009 | `test_page_table_map` | Map virtual to physical | ⬜ |
| M010 | `test_page_table_unmap` | Unmap pages | ⬜ |
| M011 | `test_page_fault_handler` | Demand paging | ⬜ |
| M012 | `test_heap_grow` | Kernel heap expansion | ⬜ |

### Process Management Tests
**File**: `kernel/src/tests/process_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| P001 | `test_process_create` | Create new process | ⬜ |
| P002 | `test_process_fork` | Fork process | ⬜ |
| P003 | `test_process_exec` | Execute program | ⬜ |
| P004 | `test_process_exit` | Process exit | ⬜ |
| P005 | `test_process_wait` | Wait for child | ⬜ |
| P006 | `test_process_kill` | Kill process | ⬜ |
| P007 | `test_thread_create` | Create thread | ⬜ |
| P008 | `test_thread_join` | Join thread | ⬜ |
| P009 | `test_signal_send` | Send signal | ⬜ |
| P010 | `test_signal_handle` | Signal handler | ⬜ |

### Scheduler Tests
**File**: `kernel/src/tests/scheduler_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| S001 | `test_scheduler_round_robin` | Basic scheduling | ⬜ |
| S002 | `test_scheduler_priority` | Priority scheduling | ⬜ |
| S003 | `test_scheduler_preemption` | Preemptive switch | ⬜ |
| S004 | `test_scheduler_sleep` | Sleep/wake | ⬜ |
| S005 | `test_scheduler_yield` | Voluntary yield | ⬜ |
| S006 | `test_scheduler_affinity` | CPU affinity | ⬜ |
| S007 | `test_scheduler_fairness` | Fair time distribution | ⬜ |

### IPC Tests
**File**: `kernel/src/tests/ipc_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| I001 | `test_channel_create` | Create IPC channel | ⬜ |
| I002 | `test_channel_send` | Send message | ⬜ |
| I003 | `test_channel_recv` | Receive message | ⬜ |
| I004 | `test_channel_blocking` | Blocking recv | ⬜ |
| I005 | `test_shm_create` | Create shared memory | ⬜ |
| I006 | `test_shm_map` | Map shared memory | ⬜ |
| I007 | `test_shm_sync` | Synchronized access | ⬜ |
| I008 | `test_mqueue_send` | Message queue send | ⬜ |
| I009 | `test_mqueue_recv` | Message queue receive | ⬜ |
| I010 | `test_mqueue_priority` | Priority messages | ⬜ |

### Syscall Tests
**File**: `kernel/src/tests/syscall_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| SYS001 | `test_sys_read` | Read syscall | ⬜ |
| SYS002 | `test_sys_write` | Write syscall | ⬜ |
| SYS003 | `test_sys_open` | Open syscall | ⬜ |
| SYS004 | `test_sys_close` | Close syscall | ⬜ |
| SYS005 | `test_sys_mmap` | Memory map syscall | ⬜ |
| SYS006 | `test_sys_munmap` | Memory unmap syscall | ⬜ |
| SYS007 | `test_sys_fork` | Fork syscall | ⬜ |
| SYS008 | `test_sys_exec` | Exec syscall | ⬜ |
| SYS009 | `test_sys_exit` | Exit syscall | ⬜ |
| SYS010 | `test_sys_getpid` | GetPID syscall | ⬜ |
| SYS011 | `test_sys_invalid` | Invalid syscall number | ⬜ |
| SYS012 | `test_sys_permission` | Permission denied | ⬜ |

### Filesystem Tests
**File**: `kernel/src/tests/fs_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| F001 | `test_vfs_mount` | Mount filesystem | ⬜ |
| F002 | `test_vfs_unmount` | Unmount filesystem | ⬜ |
| F003 | `test_file_create` | Create file | ⬜ |
| F004 | `test_file_delete` | Delete file | ⬜ |
| F005 | `test_file_read` | Read file | ⬜ |
| F006 | `test_file_write` | Write file | ⬜ |
| F007 | `test_file_seek` | Seek in file | ⬜ |
| F008 | `test_dir_create` | Create directory | ⬜ |
| F009 | `test_dir_list` | List directory | ⬜ |
| F010 | `test_path_resolve` | Path resolution | ⬜ |

### Security Tests
**File**: `kernel/src/tests/security_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| SEC001 | `test_capability_create` | Create capability | ⬜ |
| SEC002 | `test_capability_check` | Check capability | ⬜ |
| SEC003 | `test_capability_revoke` | Revoke capability | ⬜ |
| SEC004 | `test_sandbox_create` | Create sandbox | ⬜ |
| SEC005 | `test_sandbox_restrict` | Restrict syscalls | ⬜ |
| SEC006 | `test_audit_log` | Security audit log | ⬜ |

---

## 5.2.2 Browser Unit Tests

### Tab Management Tests
**File**: `kpio-browser/src/tests/tab_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| T001 | `test_tab_create` | Create new tab | ⬜ |
| T002 | `test_tab_close` | Close tab | ⬜ |
| T003 | `test_tab_switch` | Switch active tab | ⬜ |
| T004 | `test_tab_duplicate` | Duplicate tab | ⬜ |
| T005 | `test_tab_move` | Move tab position | ⬜ |
| T006 | `test_tab_pin` | Pin/unpin tab | ⬜ |
| T007 | `test_tab_mute` | Mute tab audio | ⬜ |
| T008 | `test_tab_suspend` | Suspend inactive tab | ⬜ |

### Navigation Tests
**File**: `kpio-browser/src/tests/navigation_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| N001 | `test_url_parse` | Parse URL | ⬜ |
| N002 | `test_url_normalize` | Normalize URL | ⬜ |
| N003 | `test_history_push` | Add to history | ⬜ |
| N004 | `test_history_back` | Navigate back | ⬜ |
| N005 | `test_history_forward` | Navigate forward | ⬜ |
| N006 | `test_redirect_follow` | Follow redirects | ⬜ |
| N007 | `test_redirect_limit` | Max redirects | ⬜ |

### Cookie Tests
**File**: `kpio-browser/src/tests/cookie_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| C001 | `test_cookie_set` | Set cookie | ⬜ |
| C002 | `test_cookie_get` | Get cookie | ⬜ |
| C003 | `test_cookie_expire` | Cookie expiration | ⬜ |
| C004 | `test_cookie_domain` | Domain matching | ⬜ |
| C005 | `test_cookie_path` | Path matching | ⬜ |
| C006 | `test_cookie_secure` | Secure flag | ⬜ |
| C007 | `test_cookie_httponly` | HttpOnly flag | ⬜ |
| C008 | `test_cookie_samesite` | SameSite policy | ⬜ |

### CSP Tests
**File**: `kpio-browser/src/tests/csp_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| CSP001 | `test_csp_parse` | Parse CSP header | ⬜ |
| CSP002 | `test_csp_script_src` | script-src directive | ⬜ |
| CSP003 | `test_csp_style_src` | style-src directive | ⬜ |
| CSP004 | `test_csp_img_src` | img-src directive | ⬜ |
| CSP005 | `test_csp_default_src` | default-src fallback | ⬜ |
| CSP006 | `test_csp_nonce` | Nonce validation | ⬜ |
| CSP007 | `test_csp_hash` | Hash validation | ⬜ |
| CSP008 | `test_csp_report` | Violation reporting | ⬜ |

### Extension Tests
**File**: `kpio-browser/src/tests/extension_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| E001 | `test_extension_load` | Load extension | ⬜ |
| E002 | `test_extension_unload` | Unload extension | ⬜ |
| E003 | `test_extension_enable` | Enable/disable | ⬜ |
| E004 | `test_extension_permission` | Check permissions | ⬜ |
| E005 | `test_content_script` | Inject content script | ⬜ |
| E006 | `test_extension_storage` | Extension storage | ⬜ |

### Private Mode Tests
**File**: `kpio-browser/src/tests/private_mode_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| PM001 | `test_private_session` | Create private session | ⬜ |
| PM002 | `test_private_isolation` | Session isolation | ⬜ |
| PM003 | `test_private_no_history` | No history saved | ⬜ |
| PM004 | `test_private_cookies` | Temp cookies only | ⬜ |
| PM005 | `test_private_cleanup` | Cleanup on close | ⬜ |

---

## 5.2.3 App Unit Tests

### Desktop Tests
**File**: `kpio-browser/src/apps/tests/desktop_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| D001 | `test_window_create` | Create window | ⬜ |
| D002 | `test_window_move` | Move window | ⬜ |
| D003 | `test_window_resize` | Resize window | ⬜ |
| D004 | `test_window_minimize` | Minimize window | ⬜ |
| D005 | `test_window_maximize` | Maximize window | ⬜ |
| D006 | `test_window_snap` | Snap to edge | ⬜ |
| D007 | `test_wallpaper_set` | Set wallpaper | ⬜ |
| D008 | `test_icon_arrange` | Arrange icons | ⬜ |

### File Explorer Tests
**File**: `kpio-browser/src/apps/tests/file_explorer_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| FE001 | `test_navigate_path` | Navigate to path | ⬜ |
| FE002 | `test_navigate_back` | Navigate back | ⬜ |
| FE003 | `test_select_file` | Select file | ⬜ |
| FE004 | `test_select_multiple` | Multi-select | ⬜ |
| FE005 | `test_copy_file` | Copy to clipboard | ⬜ |
| FE006 | `test_paste_file` | Paste from clipboard | ⬜ |
| FE007 | `test_delete_file` | Delete file | ⬜ |
| FE008 | `test_rename_file` | Rename file | ⬜ |
| FE009 | `test_search_files` | Search files | ⬜ |
| FE010 | `test_sort_files` | Sort by name/date/size | ⬜ |

### Terminal Tests
**File**: `kpio-browser/src/apps/tests/terminal_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| TM001 | `test_command_echo` | Echo command | ⬜ |
| TM002 | `test_command_cd` | Change directory | ⬜ |
| TM003 | `test_command_pwd` | Print directory | ⬜ |
| TM004 | `test_command_history` | Command history | ⬜ |
| TM005 | `test_command_clear` | Clear screen | ⬜ |
| TM006 | `test_env_get` | Get env variable | ⬜ |
| TM007 | `test_env_set` | Set env variable | ⬜ |

### Text Editor Tests
**File**: `kpio-browser/src/apps/tests/text_editor_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| TE001 | `test_insert_char` | Insert character | ⬜ |
| TE002 | `test_delete_char` | Delete character | ⬜ |
| TE003 | `test_newline` | Insert newline | ⬜ |
| TE004 | `test_undo` | Undo action | ⬜ |
| TE005 | `test_redo` | Redo action | ⬜ |
| TE006 | `test_select_text` | Select text | ⬜ |
| TE007 | `test_copy_paste` | Copy/paste text | ⬜ |
| TE008 | `test_find_text` | Find text | ⬜ |
| TE009 | `test_replace_text` | Replace text | ⬜ |
| TE010 | `test_syntax_highlight` | Syntax highlighting | ⬜ |

### Calculator Tests
**File**: `kpio-browser/src/apps/tests/calculator_tests.rs`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| CA001 | `test_add` | Addition | ⬜ |
| CA002 | `test_subtract` | Subtraction | ⬜ |
| CA003 | `test_multiply` | Multiplication | ⬜ |
| CA004 | `test_divide` | Division | ⬜ |
| CA005 | `test_divide_zero` | Division by zero | ⬜ |
| CA006 | `test_sqrt` | Square root | ⬜ |
| CA007 | `test_power` | Power function | ⬜ |
| CA008 | `test_sin` | Sine function | ⬜ |
| CA009 | `test_cos` | Cosine function | ⬜ |
| CA010 | `test_memory` | Memory functions | ⬜ |

---

## 5.2.4 Parser Unit Tests

### HTML Parser Tests
**File**: `kpio-html/src/tests/`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| H001 | `test_doctype` | Parse DOCTYPE | ⬜ |
| H002 | `test_simple_element` | Parse `<div>` | ⬜ |
| H003 | `test_nested_elements` | Nested tags | ⬜ |
| H004 | `test_attributes` | Parse attributes | ⬜ |
| H005 | `test_text_content` | Text nodes | ⬜ |
| H006 | `test_comments` | HTML comments | ⬜ |
| H007 | `test_void_elements` | Self-closing tags | ⬜ |
| H008 | `test_malformed` | Error recovery | ⬜ |

### CSS Parser Tests
**File**: `kpio-css/src/tests/`

| Test ID | Test Name | Description | Status |
|---------|-----------|-------------|--------|
| CS001 | `test_selector_tag` | Tag selector | ⬜ |
| CS002 | `test_selector_class` | Class selector | ⬜ |
| CS003 | `test_selector_id` | ID selector | ⬜ |
| CS004 | `test_selector_compound` | Compound selector | ⬜ |
| CS005 | `test_property_color` | Color property | ⬜ |
| CS006 | `test_property_length` | Length units | ⬜ |
| CS007 | `test_media_query` | Media queries | ⬜ |
| CS008 | `test_specificity` | Specificity calc | ⬜ |

---

## Test Infrastructure

### Test Framework Setup
```rust
// kernel/src/tests/mod.rs
#![cfg(test)]

mod memory_tests;
mod process_tests;
mod scheduler_tests;
mod ipc_tests;
mod syscall_tests;
mod fs_tests;
mod security_tests;

// Test utilities
pub mod test_utils {
    pub fn setup_test_env() { ... }
    pub fn cleanup_test_env() { ... }
    pub fn mock_process() -> Process { ... }
}
```

### Coverage Commands
```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run with coverage
cargo tarpaulin --all --out Html --output-dir coverage/

# View coverage report
open coverage/tarpaulin-report.html
```

### CI Integration
```yaml
# .github/workflows/test.yml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v3
    - name: Run tests
      run: cargo test --all
    - name: Coverage
      run: cargo tarpaulin --all --out Xml
    - name: Upload coverage
      uses: codecov/codecov-action@v3
```

---

## Acceptance Criteria

- [ ] All critical tests pass (M*, P*, SYS*)
- [ ] Coverage > 75% for kernel
- [ ] Coverage > 70% for browser
- [ ] No flaky tests
- [ ] Tests run in < 5 minutes

---

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| QA | | | |
