# Suisho 水晶 — math workbench

*Suishō* (水晶, rock crystal): a quartz crystal keeps the time in every calculator. Part of the
[Serpantinum Glass](https://github.com/xDen4ik247/serpantinum/tree/glass) app family; its installer offers it as an option.

An exact calculator for math students, with step-by-step solutions the way a textbook does them (not machine
algorithms), matrices, analytic geometry and a Desmos-style grapher. A liquid-glass quickshell window (`qml/`) driven
by a Rust engine (`engine/`, `suisho-engine`) over JSON lines.

Open it with **Mod+=** (or the calculator key) or `suisho`.

## Now (M1–M4)
- **Rows like Desmos**: every row is an expression, a definition (`a = 3`, `f(x) = x^2 + a`, in any order), a matrix
  or a graph (`y = sin x`, `x^2 + y^2 < 4`). Enter adds a row, ↑↓ move, Backspace on an empty row removes it.
- **Exact arithmetic**: fractions stay fractions (`1/3 + 1/6 = 1/2`), big integers stay exact (`2^100`), with a decimal
  next to it. Roots come out exact when they are (`8^(2/3) = 4`, `sqrt(16/9) = 4/3`).
- **Input the way you write it**: `2x`, `xy`, `sin 2x`, `sin^2 x`, `|x - 1|`, `5!`, `log_2 8`, `tg`/`ctg`/`arctg`,
  `30°`; `sqrt` → √, `pi` → π, `<=` → ≤ while typing; pasted LaTeX (`\frac{1}{2}`, `\sqrt{x}`) works.
- **Matrices**: `[[2,1],[1,3]]` or `[2,1;1,3]`, `det`, `inv`/`A^-1`, `A^T`, `rref`, `rank`, `tr`, `eye(n)`, products and
  powers, all exact.
- Lists (`[1,2,3]`, `mean`, `median`, `total`), points `(1,2)`, piecewise `{x<0: -x, x}`, restrictions `{x>0}`,
  `gcd`, `lcm`, `mod`, `nCr`, `nPr`.
- **Exact symbolic answers**: `√8 = 2√2`, `1/√2 = √2/2`, `π/2 + π/3 = 5π/6`, `cos(π/6) = √3/2`, `2x + 3x = 5x`,
  drawn as real math (fractions, roots, matrices).
- **Step-by-step solutions** (the *steps* button, checked ✓ against the engine's own answer): determinants (2×2
  formula, rule of Sarrus, cofactor expansion along the row/column with the most zeros, triangular form for big
  ones), the inverse (2×2 formula or Gauss-Jordan on [A | I] with every row operation `R₂ ← R₂ − 3R₁`), rank and
  rref, sums of fractions over the least common denominator, simplifying roots. EN/RU.
- **Equations and inequalities** in one unknown, typed as they are (`x^2 - 5x + 6 = 0`, `x^3 - 2x^2 - 5x + 6 = 0`,
  `x^2 - x - 6 > 0`): exact roots with steps — linear, quadratic (x² = k, taking x out, or the discriminant with
  exact roots like `−3/4 ± √41/4`), biquadratic (t = x²), higher degree by the rational-root theorem + Horner's
  scheme; inequalities by the interval method. `factor(x^3 - x) = x(x − 1)(x + 1)`, `expand((x - 2)^3)`.
- **Grapher** (Desmos-style): `y = f(x)`, `x = g(y)`, any implicit curve `F(x, y) = G(x, y)` (`x² + y² = 9`,
  `sin(xy) = cos y`), inequalities with shading (dashed edge for < >), polar `r = 2cos(3θ)`, parametric
  `(cos t, sin t)` (range from `{0 < t < 1}`), points, restrictions `{x > 0}`, piecewise. Asymptotes are not joined.
  Drag to pan, wheel/pinch to zoom (instant: geometry is re-sampled in the background), +/−/⌂ buttons.
  **Sliders**: a row `a = 2` gets a slider with ▶ play; an undefined letter in a graph offers “+ slider a”.
  **Points of interest** of the row you edit: zeros, extrema, y-intercept, intersections (click to pin).
- Workspaces autosave (`~/.local/share/suisho/workspaces/`), EN/RU (Ctrl+L), RAD/DEG (Ctrl+D), Ctrl+N new, Ctrl+O list.

## Coming next (the plan)
M3 rest: GPU shaders for implicit plots, trace,
export · M4 rest: rational equations with
the domain, |x|, systems · M5 linear algebra with steps (Sarrus, cofactor expansion, Gauss-Jordan
row by row, eigenvalues) + a linear-map animator · M6 calculus (derivatives, integrals, limits, full function
investigation) · M7 analytic geometry 2D/3D (conics to canonical form) · M8 stats, units, bases · M9 trainer ·
M10 structural editor.

## Build / install
`./install.sh` (needs `cargo`): builds the engine and installs to `~/.local/share/suisho/app`, `~/.local/bin/suisho`.
Your workspaces and `~/.config/suisho/settings.json` stay; `./uninstall.sh` removes the program only.
Tests: `cargo test --manifest-path engine/Cargo.toml`.

Code: GPL-3.0-or-later (`LICENSE`).
