# Recursor

**Automatically bounce back to what you were doing while Cursor works.**

---

## Install

Open your terminal and run:

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

## Commands

```bash
recursor status       # See what window is saved
recursor permissions  # Fix macOS permissions
recursor clear        # Reset saved state
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
Go to System Preferences → Privacy & Security → Accessibility → Enable Terminal or Recursor

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

## License

MIT
