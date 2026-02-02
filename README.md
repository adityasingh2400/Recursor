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

1. You're browsing YouTube
2. You switch to Cursor and submit a prompt
3. **Recursor sends you back to YouTube** while the AI works
4. **Cursor pops back up** when the AI is done

No more staring at loading screens.

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
