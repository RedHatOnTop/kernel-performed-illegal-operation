# KPIO OS Quick Start Guide

## 🚀 Get Started in 5 Minutes

### Step 1: Boot KPIO OS

1. Insert the KPIO OS USB or CD
2. Start your computer
3. Select KPIO OS from the boot menu
4. Wait for the desktop to appear

### Step 2: Explore the Desktop

![Desktop Layout]

- **Taskbar** (bottom) - Your main navigation hub
- **Start Menu** - Click the KPIO logo or press `Super` key
- **Desktop** - Right-click for options

### Step 3: Launch the Browser

1. Click the **Browser** icon on the taskbar
2. Or press `Super` and type "Browser"
3. Enter a URL in the address bar

### Step 4: Open Applications

**From Start Menu:**
1. Press `Super` key
2. Type the app name
3. Press `Enter`

**Quick Access:**
- 📁 File Manager: `Super + E`
- 💻 Terminal: `Ctrl + Alt + T`
- ⚙️ Settings: Right-click desktop → Settings

### Step 5: Customize Your Experience

1. Right-click the desktop
2. Choose "Settings"
3. Select "Appearance"
4. Pick your theme and wallpaper

---

## 📌 Essential Shortcuts

| Do This | Press This |
|---------|------------|
| Open Start Menu | `Super` |
| Switch Windows | `Alt + Tab` |
| Close Window | `Alt + F4` |
| New Browser Tab | `Ctrl + T` |
| Lock Screen | `Super + L` |

---

## Developer Boot Guide (QEMU)

If you are developing or testing KPIO OS locally with QEMU, **UEFI pflash** is the
recommended boot method. All run scripts default to UEFI mode.

```powershell
# Recommended — UEFI pflash boot (default)
.\scripts\run-qemu.ps1

# Automated testing
.\scripts\qemu-test.ps1 -Mode boot
```

> **⚠️ BIOS boot is not recommended.** The external `bootloader` crate (v0.11.14)
> contains a known FAT parser overflow that causes panics in debug builds.
> See [Known Issues](known-issues.md) for details.

---

## 🆘 Need Help?

- Press `F1` for in-app help
- Visit Settings → About for system info
- Hold `Shift` during boot for Recovery Mode
- See [Known Issues](known-issues.md) for boot problems and workarounds

---

**Welcome to KPIO OS!** 🎉
