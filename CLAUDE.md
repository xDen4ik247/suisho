# Suisho 水晶 — notes for Claude

Math workbench (calculator + step-by-step solutions + matrices + geometry + grapher). One app of the Serpantinum
Glass family: its own repo (github.com/xDen4ik247/suisho), shipped as an option by the serpantinum-glass installer.
The full plan (milestones M1–M10, architecture, protocol) is in the user's plan file; README lists the milestones.

## Rules (user decisions)
- **Rust only** for all logic (`engine/`, crate `suisho-engine`); UI is QML (quickshell); tiny shell glue is fine.
  **No Python.**
- **Install what you finish.** After a change is verified, run `./install.sh` without being asked; an open window keeps
  the old code: tell the user to reopen it.
- Step-by-step solutions use textbook methods (Sarrus, cofactor expansion, Gauss-Jordan with each row operation,
  discriminant, rational roots, substitution/by parts…), and every shown step must be checked numerically.
- `.` is always the decimal point and `,` always separates (RU only shows a decimal comma in output).
- No personal data in the repo; official Arch packages only (math fonts: otf-libertinus, otf-latinmodern-math).

## Layout and paths
- Engine: `num.rs` exact numbers, `parse.rs` input → syntax tree (Desmos rules), `eval.rs` values/matrices/builtins,
  `doc.rs` rows + workspace, `main.rs` JSON-lines protocol (`{cmd, rid}` → `{type, rid}`).
- Data `~/.local/share/suisho/workspaces/*.json` (`SUISHO_DATA`), settings `~/.config/suisho/settings.json`
  (`SUISHO_CONFIG`).
- Window: app-id `suisho`, title "Suisho" (tests: "Suisho (test)", never steals focus), IPC target `suisho`
  (`shot <png>`, `state`, `add <text>`, `fresh`).
- QML gotcha: pass shared objects as `theme: shell.themeRef` (a `theme: theme` binding inside a component binds to
  itself); don't name ListModel roles `data`.
