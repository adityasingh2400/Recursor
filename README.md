# Recursor

**The "Bounce Back" Utility for Cursor AI Agents**

Stop watching Cursor think. Recursor automatically returns you to what you were doing while the AI works, then brings Cursor back when it's ready.

---

## One-Line Install

```bash
curl -fsSL https://raw.githubusercontent.com/adityasingh2400/Recursor/main/install.sh | sh
```

That's it. Restart Cursor and you're done.

---

## What It Does

```
You're on YouTube → Switch to Cursor → Submit prompt → 
  ↓
Recursor saves "YouTube" → Returns you to YouTube →
  ↓
AI works in background → You keep watching →
  ↓
AI finishes → Cursor pops back to foreground
```

**No more staring at a loading screen.** Continue browsing, coding, or whatever you were doing.

---

## Features

- **Zero config** - One command install, works immediately
- **Cross-platform** - macOS, Windows, Linux
- **Smart** - Won't interrupt if you switched apps manually
- **Lightweight** - Single 2MB binary, no dependencies

---

## How It Works

Recursor uses [Cursor Hooks](https://cursor.com/docs/agent/hooks) to:

1. **`beforeSubmitPrompt`** - Save your current window, return focus to it
2. **`stop`** - Bring Cursor back when the agent finishes

---

## Manual Installation

If you prefer not to use the install script:

```bash
# 1. Download for your platform from Releases
# 2. Move to ~/.cursor/bin/
mkdir -p ~/.cursor/bin
mv recursor ~/.cursor/bin/
chmod +x ~/.cursor/bin/recursor

# 3. Create hooks.json
cat > ~/.cursor/hooks.json << 'EOF'
{
  "version": 1,
  "hooks": {
    "beforeSubmitPrompt": [
      { "command": "$HOME/.cursor/bin/recursor save" }
    ],
    "stop": [
      { "command": "$HOME/.cursor/bin/recursor restore" }
    ]
  }
}
EOF

# 4. Restart Cursor
```

---

## Commands

```bash
recursor status       # Show saved window state
recursor permissions  # Check/fix macOS permissions
recursor clear        # Clear saved state
recursor save         # Manually save (used by hook)
recursor restore      # Manually restore (used by hook)
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

## Platform Notes

| Platform | Notes |
|----------|-------|
| **macOS** | Requires Accessibility permission (prompted on first run) |
| **Windows** | Works out of the box |
| **Linux** | Requires X11. Install `xdotool`: `sudo apt install xdotool` |

---

## Troubleshooting

**macOS: Window switching doesn't work**
→ System Preferences → Privacy & Security → Accessibility → Enable Terminal/Recursor

**Cursor doesn't come back**
→ Run `recursor status` to check if state is being saved
→ Verify `~/.cursor/hooks.json` exists and is correct

---

## License

MIT

---

**Made for the Cursor community** ✨
