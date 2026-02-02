# Recursor

**Switch back to what you were doing while Cursor works. Get pulled back when it needs you.**

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

## macOS Setup (Required)

macOS needs Accessibility permissions to switch windows. **You must enable this or Recursor won't work.**

1. Open **System Settings** → **Privacy & Security** → **Accessibility**
2. Click the **+** button
3. Add **Terminal** (or whatever terminal app you use)
4. Make sure the toggle is **ON**

To verify it's working:
```bash
recursor permissions
```

If you see "OK" for window access, you're good.

---

## How It Works

Recursor hooks into Cursor's event system:

| Event | What Happens |
|-------|--------------|
| You submit a prompt | Recursor saves your current window and sends you back to it |
| Agent runs a command | If it needs approval, pulls you to Cursor. Otherwise runs silently. |
| Command finishes | Sends you back to your window |
| Agent finishes | Pulls you to Cursor to see the results |

---

## Manual Setup

The installer sets up `~/.cursor/hooks.json` automatically. If you need to do it manually:

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

Replace `YOUR_USERNAME` with your actual username (`whoami` to find it).

---

## Requirements

| Platform | What You Need |
|----------|---------------|
| macOS | Enable Accessibility permission (see above) |
| Windows | Nothing extra |
| Linux | Install xdotool: `sudo apt install xdotool` |

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
You need to enable Accessibility permissions. System Settings → Privacy & Security → Accessibility → Add and enable Terminal.

**Not getting pulled back to Cursor?**  
Run `recursor status` to see if state is being saved properly.

**YouTube not pausing/resuming?**  
Only works with Chrome right now.

**Commands running without asking for approval?**  
That command is in your Cursor allowlist. Check Cursor settings → Agent → Command Allowlist.

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
