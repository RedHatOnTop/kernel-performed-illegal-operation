# Sub-Phase 5.3: Integration & End-to-End Testing Checklist

## Overview
Test complete user workflows and component interactions to ensure system works as a whole.

---

## 5.3.1 Boot Sequence Tests

### Test Scenarios

| Test ID | Scenario | Steps | Expected Result | Status |
|---------|----------|-------|-----------------|--------|
| BOOT001 | Cold Boot | Power on → BIOS → Bootloader → Kernel → Desktop | Desktop appears in < 3s | ⬜ |
| BOOT002 | Warm Reboot | Click Restart → System reboots → Desktop | System recovers state | ⬜ |
| BOOT003 | Multi-Display | Boot with 2 monitors | Both displays initialized | ⬜ |
| BOOT004 | Network Boot | Boot with ethernet connected | Network available at login | ⬜ |
| BOOT005 | Recovery Mode | Hold F8 → Recovery menu | Recovery options shown | ⬜ |
| BOOT006 | Safe Mode | Boot in safe mode | Minimal drivers loaded | ⬜ |
| BOOT007 | Boot Failure | Corrupt kernel image | Recovery prompt shown | ⬜ |

### Boot Timing Breakdown

```
Target: Total < 3 seconds

Phase             Target    Actual    Status
─────────────────────────────────────────────
UEFI Init         500ms     ___ms     ⬜
Bootloader        200ms     ___ms     ⬜
Kernel Init       800ms     ___ms     ⬜
Driver Load       500ms     ___ms     ⬜
Desktop Ready     1000ms    ___ms     ⬜
─────────────────────────────────────────────
TOTAL             3000ms    ___ms     ⬜
```

---

## 5.3.2 Desktop Workflow Tests

### Test Scenarios

| Test ID | Scenario | Steps | Expected Result | Status |
|---------|----------|-------|-----------------|--------|
| DW001 | Full Lifecycle | Boot → Desktop → Launch App → Close → Shutdown | Clean shutdown | ⬜ |
| DW002 | Multi-Window | Open 5 windows → Arrange → Close all | No memory leak | ⬜ |
| DW003 | Window Snap | Drag window to edge | Window snaps correctly | ⬜ |
| DW004 | Taskbar Click | Click taskbar app icon | Window focuses/minimizes | ⬜ |
| DW005 | System Tray | Click battery/wifi/volume | Popup shows | ⬜ |
| DW006 | Quick Settings | Open quick settings | All toggles work | ⬜ |
| DW007 | Notifications | Trigger notification | Toast appears, dismissable | ⬜ |
| DW008 | Search | Press Super key → Type | Search results appear | ⬜ |

### Detailed Test: DW001 Full Lifecycle

```
Step  Action                      Verify
────────────────────────────────────────────────────────
1     System boots               Desktop appears
2     Click AppLauncher          Launcher opens
3     Search "Calculator"        Calculator shown
4     Click Calculator           Calculator window opens
5     Press 2+2=                 Display shows 4
6     Click X button             Window closes
7     Click power icon           Shutdown menu appears
8     Click Shutdown             System powers off
────────────────────────────────────────────────────────
```

---

## 5.3.3 Browser Workflow Tests

### Test Scenarios

| Test ID | Scenario | Steps | Expected Result | Status |
|---------|----------|-------|-----------------|--------|
| BW001 | Basic Browse | Open browser → Navigate → Close | Page loads correctly | ⬜ |
| BW002 | Multi-Tab | Open 5 tabs → Switch between | All tabs responsive | ⬜ |
| BW003 | Private Mode | Open private window → Browse → Close | No history saved | ⬜ |
| BW004 | Bookmark | Add bookmark → Close → Reopen → Access | Bookmark persists | ⬜ |
| BW005 | Download | Download file → Open downloads → Open file | File saved, opens | ⬜ |
| BW006 | Extension | Install extension → Verify working | Extension active | ⬜ |
| BW007 | Form Submit | Fill form → Submit | Data sent correctly | ⬜ |
| BW008 | Media Play | Open video page → Play video | Video plays with audio | ⬜ |

### Detailed Test: BW001 Basic Browse

```
Step  Action                      Verify
────────────────────────────────────────────────────────
1     Click browser in taskbar   Browser window opens
2     Click address bar          Address bar focused
3     Type "example.com"         Text appears
4     Press Enter                Loading indicator shows
5     Wait for load              Page content visible
6     Scroll down                Page scrolls smoothly
7     Click link                 Navigation occurs
8     Click back button          Previous page shown
9     Click X button             Browser closes
────────────────────────────────────────────────────────
```

---

## 5.3.4 File Management Tests

### Test Scenarios

| Test ID | Scenario | Steps | Expected Result | Status |
|---------|----------|-------|-----------------|--------|
| FM001 | Create File | New → Text File → Name → Save | File appears in dir | ⬜ |
| FM002 | Edit File | Open file → Edit → Save | Changes persisted | ⬜ |
| FM003 | Copy File | Select → Copy → Navigate → Paste | File copied | ⬜ |
| FM004 | Move File | Select → Cut → Navigate → Paste | File moved | ⬜ |
| FM005 | Delete File | Select → Delete | File in trash | ⬜ |
| FM006 | Restore File | Open trash → Restore | File back in place | ⬜ |
| FM007 | Search | Type in search → Enter | Results shown | ⬜ |
| FM008 | Properties | Right-click → Properties | Info dialog shows | ⬜ |

### Detailed Test: FM001 Create File

```
Step  Action                      Verify
────────────────────────────────────────────────────────
1     Open File Explorer         Explorer window opens
2     Navigate to Documents      Documents folder shown
3     Right-click empty space    Context menu appears
4     Click New → Text File      New file created
5     Type "notes.txt"           Name updated
6     Press Enter                File created
7     Double-click file          Text Editor opens
8     Type "Hello World"         Text appears
9     Press Ctrl+S               File saved
10    Close Text Editor          Editor closes
11    Check file in Explorer     File shows in list
────────────────────────────────────────────────────────
```

---

## 5.3.5 Settings Tests

### Test Scenarios

| Test ID | Scenario | Steps | Expected Result | Status |
|---------|----------|-------|-----------------|--------|
| ST001 | Change Theme | Settings → Theme → Dark | UI updates to dark | ⬜ |
| ST002 | Change Wallpaper | Settings → Background → Select | Wallpaper changes | ⬜ |
| ST003 | Change Language | Settings → Language → Select | UI text changes | ⬜ |
| ST004 | Change Timezone | Settings → Time → Select | Clock updates | ⬜ |
| ST005 | Privacy Toggle | Settings → Privacy → Toggle | Setting persists | ⬜ |
| ST006 | Sound Volume | Settings → Sound → Adjust | Volume changes | ⬜ |
| ST007 | Display Scale | Settings → Display → Scale | UI scales | ⬜ |
| ST008 | Reset Settings | Settings → Reset | Defaults restored | ⬜ |

---

## 5.3.6 App Integration Tests

### Calculator Integration

| Test ID | Scenario | Expected Result | Status |
|---------|----------|-----------------|--------|
| CALC001 | Basic math | 123 + 456 = 579 | ⬜ |
| CALC002 | Decimal | 1.5 × 2 = 3 | ⬜ |
| CALC003 | Scientific | sin(90°) = 1 | ⬜ |
| CALC004 | Memory | M+ → MC → MR | ⬜ |
| CALC005 | History | Previous calculations shown | ⬜ |

### Terminal Integration

| Test ID | Scenario | Expected Result | Status |
|---------|----------|-----------------|--------|
| TERM001 | Echo | `echo hello` → "hello" | ⬜ |
| TERM002 | Navigation | `cd /tmp && pwd` → "/tmp" | ⬜ |
| TERM003 | Environment | `env` shows variables | ⬜ |
| TERM004 | History | Up arrow recalls command | ⬜ |
| TERM005 | Clear | `clear` clears screen | ⬜ |

### Text Editor Integration

| Test ID | Scenario | Expected Result | Status |
|---------|----------|-----------------|--------|
| EDIT001 | Open file | File content shown | ⬜ |
| EDIT002 | Edit save | Changes persisted | ⬜ |
| EDIT003 | Undo redo | State restored | ⬜ |
| EDIT004 | Find | Search highlights matches | ⬜ |
| EDIT005 | Syntax | Code highlighted | ⬜ |

### Media Viewer Integration

| Test ID | Scenario | Expected Result | Status |
|---------|----------|-----------------|--------|
| MEDIA001 | Open image | Image displays | ⬜ |
| MEDIA002 | Zoom | Image zooms | ⬜ |
| MEDIA003 | Rotate | Image rotates | ⬜ |
| MEDIA004 | Slideshow | Auto-advance works | ⬜ |
| MEDIA005 | Video play | Video plays | ⬜ |

---

## E2E Test Framework

### Framework Structure

```rust
// tests/e2e/mod.rs
pub struct E2ERunner {
    qemu: QemuInstance,
    screen: ScreenCapture,
    input: InputSimulator,
}

impl E2ERunner {
    /// Boot the OS and wait for desktop
    pub async fn boot(&mut self) -> Result<()> {
        self.qemu.start()?;
        self.wait_for_element("desktop-background", 30_000).await?;
        Ok(())
    }

    /// Click at coordinates
    pub async fn click(&mut self, x: i32, y: i32) {
        self.input.mouse_move(x, y);
        self.input.mouse_click(MouseButton::Left);
        self.wait_idle().await;
    }

    /// Type text
    pub async fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.input.key_press(ch);
        }
        self.wait_idle().await;
    }

    /// Wait for element to appear
    pub async fn wait_for_element(&self, id: &str, timeout_ms: u64) -> Result<Rect>;

    /// Assert element is visible
    pub fn assert_visible(&self, id: &str) -> bool;

    /// Take screenshot
    pub fn screenshot(&self) -> Image;

    /// Compare with reference image
    pub fn assert_screenshot(&self, reference: &str, tolerance: f32) -> bool;
}
```

### Example E2E Test

```rust
#[tokio::test]
async fn test_full_workflow() {
    let mut runner = E2ERunner::new();
    
    // Boot
    runner.boot().await.unwrap();
    assert!(runner.assert_visible("taskbar"));
    
    // Open calculator
    runner.click(50, 500).await; // AppLauncher
    runner.wait_for_element("app-launcher", 1000).await.unwrap();
    runner.type_text("calc").await;
    runner.click(100, 100).await; // Calculator result
    
    // Wait for calculator window
    runner.wait_for_element("calculator-window", 2000).await.unwrap();
    
    // Do calculation
    runner.click(100, 200).await; // 2
    runner.click(150, 200).await; // +
    runner.click(100, 200).await; // 2
    runner.click(200, 300).await; // =
    
    // Verify result
    assert!(runner.assert_screenshot("calc_2plus2", 0.95));
    
    // Close
    runner.click(300, 50).await; // X button
    assert!(!runner.assert_visible("calculator-window"));
    
    // Shutdown
    runner.shutdown().await;
}
```

---

## Test Execution

### QEMU Test Environment

```bash
# Run E2E tests with QEMU
./scripts/run_e2e_tests.sh

# Run specific test
cargo test --test e2e_browser -- test_basic_browse

# Run with display (for debugging)
DISPLAY=:0 ./scripts/run_e2e_tests.sh --visible
```

### Test Matrix

| Test Suite | QEMU | VirtualBox | Hardware |
|------------|------|------------|----------|
| Boot | ✓ | ✓ | ✓ |
| Desktop | ✓ | ✓ | Pending |
| Browser | ✓ | ✓ | Pending |
| Files | ✓ | ✓ | Pending |
| Settings | ✓ | ✓ | Pending |
| Apps | ✓ | ✓ | Pending |

---

## Acceptance Criteria

- [ ] All boot tests pass
- [ ] All desktop workflow tests pass
- [ ] All browser workflow tests pass
- [ ] All file management tests pass
- [ ] All settings tests pass
- [ ] No crashes during E2E tests
- [ ] No memory leaks after full workflow

---

## Known Issues Log

| Issue ID | Description | Severity | Status |
|----------|-------------|----------|--------|
| | | | |

---

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| QA | | | |
