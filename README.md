# Recursor

**Automatically bounce back to what you were doing while Cursor works.**

---

## One-Click Install

Download and double-click:

| macOS | Windows | Linux |
|:-----:|:-------:|:-----:|
| [<img src="https://img.shields.io/badge/Download-macOS-black?style=for-the-badge&logo=apple" alt="macOS">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Mac.command) | [<img src="https://img.shields.io/badge/Download-Windows-blue?style=for-the-badge&logo=windows" alt="Windows">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Windows.bat) | [<img src="https://img.shields.io/badge/Download-Linux-orange?style=for-the-badge&logo=linux" alt="Linux">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Linux.sh) |

> **macOS**: Right-click → Open (first time only)  
> **Linux**: Right-click → Properties → Allow executing

---

## Or Use Terminal

```bash
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh
```

Then restart Cursor. Done.

---

## What It Does

Recursor intelligently manages your focus while Cursor AI agents work:

### Smart Window Switching
1. You're watching YouTube (or doing anything else)
2. You switch to Cursor and submit a prompt
3. **Recursor sends you back to YouTube** while the AI works
4. **Cursor pops back up only when needed:**
   - When the agent needs your permission to run a command
   - When the agent finishes working

### YouTube Auto Pause/Play
When you're watching YouTube and the agent needs your attention:
- **Video automatically pauses** when you're brought to Cursor
- **Video automatically resumes** when you're sent back

### Command Allowlist Integration
Recursor reads your Cursor command allowlist:
- **Allowlisted commands** run silently — you stay on YouTube
- **Non-allowlisted commands** bring you to Cursor for approval, then send you back

No more staring at loading screens. No more missing important agent prompts.

---

## How It Works

Recursor uses Cursor's hooks system to intercept key events:

| Hook | What Recursor Does |
|------|-------------------|
| `beforeSubmitPrompt` | Saves your current window, sends you back to it |
| `beforeShellExecution` | If command needs approval: pauses YouTube, brings you to Cursor |
| `afterShellExecution` | Sends you back to your window, resumes YouTube |
| `stop` | Brings you back to Cursor when agent finishes |

---

## Setup Instructions

### After Installation

The installer automatically sets up the hooks in `~/.cursor/hooks.json`. If you need to manually configure them, create this file:

```json
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [
      { "command": "/Users/YOUR_USERNAME/.cursor/bin/recursor save" }
    ],
    "beforeShellExecution": [
      { "command": "/Users/YOUR_USERNAME/.cursor/bin/recursor before-shell" }
    ],
    "afterShellExecution": [
      { "command": "/Users/YOUR_USERNAME/.cursor/bin/recursor after-shell" }
    ],
    "stop": [
      { "command": "/Users/YOUR_USERNAME/.cursor/bin/recursor restore" }
    ]
  }
}
```

Replace `YOUR_USERNAME` with your actual username (run `whoami` to find it).

### macOS Permissions

On first run, macOS will ask for Accessibility permissions. Grant them:

1. System Preferences → Privacy & Security → Accessibility
2. Enable **Terminal** (or **Cursor** if running from there)
3. Run `recursor permissions` to verify

---

## Why Recursor?

### Zero Dependencies
Written in **Rust** — compiles to a single 2MB binary. No Node.js, Python, or npm required. Just download and run.

### Zero Config  
One command installs everything. No config files to edit, no environment variables to set.

### Multi-Window Support
Have multiple Cursor windows open? Recursor tracks each one separately and returns you to the correct window.

### Cross-Platform
Works on macOS, Windows, and Linux with native system APIs.

---

## Commands

```bash
recursor status       # See saved state
recursor permissions  # Fix macOS permissions
recursor clear        # Reset state
```

---

## Requirements

| Platform | Requirement |
|----------|-------------|
| macOS | Grant Accessibility permission when prompted |
| Windows | None |
| Linux | Install xdotool: `sudo apt install xdotool` |

---

## Troubleshooting

**Not working on macOS?**  
System Preferences → Privacy & Security → Accessibility → Enable Terminal or Recursor

**Cursor not coming back?**  
Run `recursor status` to check if it's saving correctly

**YouTube not pausing/resuming?**  
Make sure Chrome is the browser you're using. Safari support coming soon.

**Commands running without bringing you to Cursor?**  
The command is probably in your allowlist. Check Cursor settings → Agent → Command Allowlist.

---

## Uninstall

```bash
rm ~/.cursor/bin/recursor
rm ~/.cursor/hooks.json
rm ~/.cursor/recursor_state.json
```

---

## Building from Source

```bash
git clone https://github.com/adityasingh2400/Recursor.git
cd Recursor
cargo build --release
cp target/release/recursor ~/.cursor/bin/
```

---

## License

MIT
