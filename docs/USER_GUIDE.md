# KPIO OS User Guide

## 1. Introduction

### What is KPIO OS?

KPIO OS is a modern, browser-based operating system designed for simplicity, security, and performance. It combines a custom kernel with a full-featured web browser to create a unified computing experience.

**Key Features:**
- 🚀 Fast boot and responsive UI
- 🌐 Full HTML5/CSS3 web browser
- 🔒 Security-first design with sandboxing
- 💻 Modern desktop environment
- 📦 Built-in system applications

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| CPU | x86_64 compatible | 2+ cores |
| RAM | 256 MB | 1 GB |
| Storage | 2 GB | 8 GB |
| Display | 1024x768 | 1920x1080 |

### First Boot

When you first boot KPIO OS, you'll be greeted with:
1. The KPIO logo and boot splash
2. The desktop environment
3. A welcome dialog with quick start tips

---

## 2. Desktop Environment

### Desktop Overview

The KPIO desktop consists of:
- **Desktop Area** - Main workspace for windows and shortcuts
- **Taskbar** - Bottom bar with app launcher and system tray
- **Start Menu** - Access to all applications

### Taskbar & Start Menu

**Taskbar Components:**
- **Start Button** - Opens the application launcher
- **Pinned Apps** - Quick access to favorite applications
- **Running Apps** - Shows open application windows
- **System Tray** - Clock, volume, network status

**Start Menu:**
- Click the Start button or press `Super` (Windows key)
- Search for applications by typing
- Access Settings and Power options

### Window Management

| Action | Mouse | Keyboard |
|--------|-------|----------|
| Move window | Drag title bar | `Alt + Drag` |
| Resize window | Drag edges | `Super + Arrow` |
| Maximize | Double-click title bar | `Super + Up` |
| Minimize | Click minimize button | `Super + Down` |
| Close | Click X button | `Alt + F4` |

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Super` | Open Start menu |
| `Alt + Tab` | Switch windows |
| `Alt + F4` | Close window |
| `Super + D` | Show desktop |
| `Super + L` | Lock screen |
| `Super + E` | Open File Manager |
| `Ctrl + Alt + T` | Open Terminal |
| `Super + Arrow` | Snap window |
| `Print Screen` | Take screenshot |

---

## 3. Web Browser

### Navigation

**Address Bar:**
- Type a URL and press `Enter` to navigate
- Type search terms to search the web
- Click the refresh button to reload the page

**Navigation Buttons:**
- ⬅️ **Back** - Go to previous page
- ➡️ **Forward** - Go to next page
- 🔄 **Refresh** - Reload current page
- 🏠 **Home** - Go to home page

### Tabs

| Action | Keyboard | Mouse |
|--------|----------|-------|
| New tab | `Ctrl + T` | Click + button |
| Close tab | `Ctrl + W` | Click X on tab |
| Reopen closed | `Ctrl + Shift + T` | Right-click → Reopen |
| Next tab | `Ctrl + Tab` | Click tab |
| Previous tab | `Ctrl + Shift + Tab` | Click tab |

### Bookmarks

**Adding Bookmarks:**
1. Navigate to the page you want to bookmark
2. Click the star icon in the address bar
3. Choose a folder or create a new one
4. Click "Done"

**Managing Bookmarks:**
- Open bookmark manager with `Ctrl + Shift + B`
- Drag and drop to organize
- Right-click to edit or delete

### Downloads

**Downloading Files:**
1. Click on a download link
2. Choose save location (or use default)
3. Monitor progress in the downloads bar

**Accessing Downloads:**
- Click downloads icon in toolbar
- Or press `Ctrl + J`
- Files are saved to `~/Downloads` by default

### Settings

Access browser settings with `Ctrl + ,` or from the menu.

**General:**
- Default search engine
- Home page
- Downloads location

**Privacy & Security:**
- Clear browsing data
- Cookie settings
- Site permissions

**Appearance:**
- Theme (Light/Dark/System)
- Font size
- Zoom level

---

## 4. System Applications

### File Manager

The file manager lets you browse and organize your files.

**Features:**
- Tree view navigation
- Grid/list view toggle
- Search files
- Create folders
- Copy/cut/paste files
- Properties viewer

**Keyboard Shortcuts:**
| Shortcut | Action |
|----------|--------|
| `Ctrl + C` | Copy |
| `Ctrl + X` | Cut |
| `Ctrl + V` | Paste |
| `Delete` | Move to trash |
| `F2` | Rename |
| `Ctrl + Shift + N` | New folder |

### Terminal

A command-line interface for advanced users.

**Features:**
- Multiple tabs
- Command history (`Up/Down` arrows)
- Copy/paste support
- Customizable colors

**Common Commands:**
```
ls          List files
cd          Change directory
pwd         Print current directory
cat         Display file contents
mkdir       Create directory
rm          Remove file
cp          Copy file
mv          Move/rename file
```

### Text Editor

A simple but powerful text editor.

**Features:**
- Syntax highlighting
- Line numbers
- Find and replace
- Multiple tabs
- Auto-save

### Calculator

A standard calculator with:
- Basic operations (+, -, ×, ÷)
- Percentage calculations
- Memory functions (M+, M-, MR, MC)
- History

### Settings

System-wide configuration application.

**Sections:**
- **General** - Language, region, date/time
- **Appearance** - Theme, wallpaper, fonts
- **Network** - Wi-Fi, ethernet settings
- **Security** - Password, lock screen
- **Privacy** - Permissions, data
- **About** - System information

---

## 5. System Settings

### Appearance

**Theme:**
- Light - Bright theme for daytime use
- Dark - Dark theme for low-light
- System - Follow system preference

**Wallpaper:**
- Choose from built-in wallpapers
- Use your own image
- Solid color option

### Network

**Wi-Fi:**
1. Click network icon in system tray
2. Select your network
3. Enter password if required
4. Click Connect

**Ethernet:**
- Automatically configured via DHCP
- Manual settings available

### Users & Accounts

**Your Account:**
- Change display name
- Set profile picture
- Change password

**Other Users:**
- Add new users
- Set permissions
- Remove users

### Storage

**View Usage:**
- See disk space breakdown
- Identify large files/folders

**Manage Storage:**
- Empty trash
- Clear cache
- Remove old files

---

## 6. Troubleshooting

### Common Issues

**System won't boot:**
1. Check if USB/CD is properly inserted
2. Verify boot order in BIOS
3. Try Recovery Mode (press `Shift` during boot)

**Slow performance:**
1. Close unused applications
2. Check for available updates
3. Clear browser cache

**Network not working:**
1. Check physical connections
2. Restart network service
3. Verify network settings

### Recovery Mode

To enter Recovery Mode:
1. Restart the computer
2. Press `Shift` repeatedly during boot
3. Select "Recovery Mode" from menu

**Recovery Options:**
- Safe Mode - Boot with minimal drivers
- Network Recovery - Boot with network support
- Reset Settings - Restore defaults
- System Restore - Restore from backup

### Getting Help

**Resources:**
- Press `F1` in any application for help
- Visit our website: https://kpio.local/help
- Community forum: https://forum.kpio.local
- Email support: support@kpio.local

---

## Keyboard Shortcut Reference

### System Shortcuts
| Shortcut | Action |
|----------|--------|
| `Super` | Open Start Menu |
| `Super + D` | Show Desktop |
| `Super + L` | Lock Screen |
| `Alt + Tab` | Switch Windows |
| `Alt + F4` | Close Window |
| `Ctrl + Alt + Del` | System Menu |
| `Print Screen` | Screenshot |

### Browser Shortcuts
| Shortcut | Action |
|----------|--------|
| `Ctrl + T` | New Tab |
| `Ctrl + W` | Close Tab |
| `Ctrl + Tab` | Next Tab |
| `Ctrl + L` | Focus Address Bar |
| `Ctrl + R` | Refresh |
| `Ctrl + D` | Bookmark Page |
| `Ctrl + F` | Find on Page |

### Text Editing
| Shortcut | Action |
|----------|--------|
| `Ctrl + C` | Copy |
| `Ctrl + X` | Cut |
| `Ctrl + V` | Paste |
| `Ctrl + Z` | Undo |
| `Ctrl + Y` | Redo |
| `Ctrl + A` | Select All |
| `Ctrl + S` | Save |

---

*KPIO OS v1.0.0*
*© 2026 KPIO Contributors*
*Licensed under MIT License*
