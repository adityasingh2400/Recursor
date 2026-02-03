# Recursor

**Switch back to what you were doing while Cursor works. Get pulled back when it needs you.**

Written in Rust. No dependencies. Just download and run.

---

## Demo

<p align="center">
  <strong>See Recursor in action.</strong><br>
  Switch to Cursor, submit a prompt, get sent back to what you were doing—and get pulled back only when the agent needs you.
</p>

https://github.com/user-attachments/assets/8b7ac0bd-58ef-43df-8a7f-f0301f72a333

---

## Install

**One command:**
```bash
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh
```

**Or download and double-click:**

| macOS | Windows | Linux |
|:-----:|:-------:|:-----:|
| [<img src="https://img.shields.io/badge/Download-macOS-black?style=for-the-badge&logo=apple" alt="macOS">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Mac.command) | [<img src="https://img.shields.io/badge/Download-Windows-blue?style=for-the-badge&logo=windows" alt="Windows">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Windows.bat) | [<img src="https://img.shields.io/badge/Download-Linux-orange?style=for-the-badge&logo=linux" alt="Linux">](https://raw.githubusercontent.com/adityasingh2400/Recursor/main/installers/Install-Recursor-Linux.sh) |

Then restart Cursor. That's it.

> **macOS**: On first use, click "Allow" when macOS asks for Accessibility permission.

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

## Optional: YouTube Auto-Pause

To have Recursor automatically pause/resume YouTube when switching windows:

**macOS**: In Chrome, go to **View → Developer → Allow JavaScript from Apple Events**

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
Click "Allow" when macOS prompts for Accessibility permission. If you missed it: System Settings → Privacy & Security → Accessibility → enable for Terminal/Cursor.

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
cargo build --release
cp target/release/recursor ~/.cursor/bin/
```

---

## License

MIT
