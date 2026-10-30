# Suisho 水晶 — math workbench

*Suishō* (水晶, rock crystal): a quartz crystal keeps the time in every calculator. Part of the
[Serpantinum Glass](https://github.com/xDen4ik247/serpantinum/tree/glass) app family; its installer offers it as an option.

An exact calculator for math students, with step-by-step solutions the way a textbook does them (not machine
algorithms), matrices, analytic geometry and a Desmos-style grapher. A liquid-glass quickshell window (`qml/`) driven
by a Rust engine (`engine/`, `suisho-engine`) over JSON lines.

Open it with **Mod+=** (or the calculator key) or `suisho`.

## Now (M1–M9)
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
- **Systems of linear equations**: `solve(2x + y = 5, x - y = 1)` — Cramer's rule (with every determinant) for
  2×2/3×3 systems, Gauss-Jordan on [A | b] otherwise; infinitely many solutions are written through the free
  unknowns (`x = 3 − 2y, y ∈ ℝ`), contradictions (0 = c) end in ∅.
- **Eigenvalues and eigenvectors**: `eig(A)` — the characteristic equation det(A − λI) = 0 solved with steps, then
  the null space of A − λI for each eigenvalue (checked A·v = λv).
- **Linear-map animator**: *show map* on any 2×2 matrix animates the grid from I to A (î, ĵ, the unit square whose
  area is det, dashed eigen-directions).
- **Calculus with steps**: `diff(x^2 sin x)` names each rule (sum, constant multiple, product, quotient, power +
  chain, exponentials, the derivative table, logarithmic differentiation); `integrate(x^2 e^x)` by linearity, the
  table, linear substitution, f′/f → ln|f|, substitution and by parts; `integrate(sin x, 0, pi)` by Newton–Leibniz
  (the area is shaded on the graph); `lim((x^2 - 1)/(x - 1), x, 1)`, `lim(sin(x)/x, x, 0)`, `lim(f, x, inf)` by
  substitution, cancelling, L'Hôpital, leading terms. Every answer is checked numerically.
- **Function investigation** `analyze(f)` (исследование функции): domain, parity, periodicity, intercepts, vertical
  and slant asymptotes (dashed on the graph), monotonicity and extrema from f′, convexity and inflection points
  from f″, with sign charts. `tangent(f, a)` draws the tangent line, `diff(f)` plots f′.
- **Second-order curves**: type any `Ax² + Bxy + Cy² + Dx + Ey + F = 0` (`x^2 + 4y^2 - 2x + 16y + 1 = 0`,
  `5x^2 + 4xy + 8y^2 = 36`): the row names the curve, *steps* reduces it to canonical form — invariants δ, Δ, S,
  completing the squares (exact semi-axes, c, eccentricity) or a rotation by tg 2θ = B/(A − C) — and the graph
  shows the foci and the directrices/asymptotes.
- **Points, lines, triangles, vectors**: `line(P, Q)` (two-point form → general → slope form), `distance(P, Q)`,
  `distance(P, 3x + y - 12 = 0)`, `midpoint(P, Q)`, `triangle(A, B, C)` (sides, perimeter, area, angles, centroid,
  circumcentre, orthocentre, incentre — exact where possible), `plane(A, B, C)`, `dot`, `cross`, `angle` — all with
  steps; lines and triangles are drawn.
- **3D surfaces**: `z = sin(x) cos(y)`, `z = (x^2 - y^2)/8`, planes like `x + y + z = 6` (anything linear in z):
  the *3D* switch (or the row's 3D chip) shows them lit and tinted by height; drag to orbit, wheel to zoom.
- **Statistics**: `mean`, `median`, `stdev`/`stdevp`, `var`, `sort`, `count`, `corr` on lists; `linreg(X, Y)` — the
  least-squares line with steps (exact for exact data), drawn with the data points; distributions `normalpdf`,
  `normalcdf(a, b, μ, σ)`, `binompdf(n, p, k)` / `binomcdf` (exact), `poissonpdf` / `poissoncdf`.
- **Units and constants**: `36 km/h to m/s`, `100 °F to °C`, `1 atm to Pa`, `5 kWh to J`, `c to km/s` (SI prefixes,
  imperial units, `G`, `gn`, `hbar`, `kB`, `NA`, `qe`, `me`…; `в` works too).
- **Bases and bits**: `0b1011`, `0o17`, `0x1F` literals, `bin(n)`, `oct(n)`, `hex(n)`, `base(n, b)`, `bitand`, `bitor`,
  `bitxor`, `shl`, `shr`.
- **Trainer** (Ctrl+T): endless practice in fractions, determinants, inverses, linear/quadratic/cubic equations,
  2×2 systems, derivatives and integrals. Problems are generated backwards so the numbers come out nice; your answer
  counts in any equivalent form (0.5 = 1/2, roots in any order, an antiderivative up to a constant); *Show solution*
  opens the full steps; a rating per topic picks the level so you get about 75 % right
  (`~/.local/share/suisho/trainer.json`).
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
the domain, |x|, systems · M7 analytic geometry 2D/3D (conics to canonical form) ·
M10 structural editor.

## Build / install
`./install.sh` (needs `cargo`): builds the engine and installs to `~/.local/share/suisho/app`, `~/.local/bin/suisho`.
Your workspaces and `~/.config/suisho/settings.json` stay; `./uninstall.sh` removes the program only.
Tests: `cargo test --manifest-path engine/Cargo.toml`.

Code: GPL-3.0-or-later (`LICENSE`).
