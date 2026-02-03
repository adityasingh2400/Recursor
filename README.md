# Recursor

**Switch back to what you were doing while Cursor works. Get pulled back when it needs you.**

Written in Rust. No dependencies. Just download and run.

---

## Demo

<p align="center">
  <strong>See Recursor in action.</strong><br>
  Switch to Cursor, submit a prompt, get sent back to what you were doing—and get pulled back only when the agent needs you.
</p>

https://github.com/user-attachments/assets/3435c638-582b-4e04-a1c9-57d8ddb46a42

---

## One-Click Install

Download and double-click:

| macOS | Windows | Linux |
|:-----:|:-------:|:-----:|
| [<img src="https://img.shields.io/badge/Download-macOS-black?style=for-the-badge&logo=apple" alt="macOS">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Mac.command) | [<img src="https://img.shields.io/badge/Download-Windows-blue?style=for-the-badge&logo=windows" alt="Windows">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Windows.bat) | [<img src="https://img.shields.io/badge/Download-Linux-orange?style=for-the-badge&logo=linux" alt="Linux">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Linux.sh) |

> **macOS**: Right-click → Open (first time only)  
> **Linux**: Right-click → Properties → Allow executing

Or use terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh
```

Then restart Cursor.

---

## What It Does

1. You're watching YouTube or doing something else
2. You switch to Cursor, submit a prompt, and the agent starts working
3. Recursor sends you back to YouTube
4. When the agent needs approval for a command, you get pulled back to Cursor
5. After you approve, you go back to YouTube
6. When the agent finishes, you get pulled back to Cursor to see the results

If you're watching YouTube, Recursor pauses the video when pulling you to Cursor and resumes it when sending you back.

---

## Setup

### macOS

You need to enable two things:

**1. Accessibility Permission** (for window switching)
- Open **System Settings** → **Privacy & Security** → **Accessibility**
- Click **+** and add **Terminal** (or your terminal app)
- Make sure the toggle is ON

**2. Chrome AppleScript** (for YouTube pause/resume)
- Open Chrome
- Go to **View** → **Developer** → **Allow JavaScript from Apple Events**
- Check the box

Run this to verify permissions are working:
```bash
recursor permissions
```

### Windows

Nothing extra needed.

### Linux

Install xdotool:
```bash
sudo apt install xdotool
```

---

## Commands

```bash
recursor status       # Check current state
recursor permissions  # Test if permissions are working (macOS)
recursor clear        # Reset saved state
```

---

## Troubleshooting

**Window switching not working on macOS?**  
Enable Accessibility permissions. System Settings → Privacy & Security → Accessibility → Add Terminal.

**YouTube not pausing/resuming?**  
Enable AppleScript in Chrome: View → Developer → Allow JavaScript from Apple Events.

**Not getting pulled back to Cursor?**  
Run `recursor status` to check if state is being saved.

**Commands running without asking for approval?**  
That command is in your Cursor allowlist. Check Cursor settings → Agent → Command Allowlist.

---

## Manual Hook Setup

The installer sets up hooks automatically. If you need to do it manually, create `~/.cursor/hooks.json`:

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

Replace `YOUR_USERNAME` with your username (`whoami` to find it).

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
git lfs pull          # fetch demo video (optional)
cargo build --release
cp target/release/recursor ~/.cursor/bin/
```

---

## License

MIT
