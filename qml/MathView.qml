import QtQuick
import QtQuick.Shapes

// Draws a math layout tree from the engine (see engine/src/mtree.rs): fractions, powers, roots, stretchy
// brackets, matrices (augmented, with highlighted rows) and highlights. Layout is done here in JS with the
// font's metrics, so it stays crisp at any scale and follows the theme live.
Item {
    id: mv
    property var tree: null
    property real size: 18
    property color color: "white"
    property color accent: "#b9c3ff"
    property string family: "DejaVu Math TeX Gyre"
    // laid out imperatively (a binding would loop: measuring writes to the TextMetrics it reads)
    property var lay: ({ w: 0, asc: 0, desc: 0, p: [] })
    function relayout() { lay = tree ? layout(tree, size) : { w: 0, asc: 0, desc: 0, p: [] } }
    onTreeChanged: relayout()
    onSizeChanged: relayout()
    onFamilyChanged: relayout()
    Component.onCompleted: relayout()
    implicitWidth: Math.ceil(lay.w) + 2
    implicitHeight: Math.ceil(lay.asc + lay.desc) + 2
    readonly property real base: lay.asc + 1

    TextMetrics { id: tm; font.family: mv.family }

    function measure(s, px) { tm.font.pixelSize = Math.max(1, Math.round(px)); tm.text = s; return tm.advanceWidth }
    function italic(s) {
        let out = "";
        for (const ch of s) {
            const c = ch.codePointAt(0);
            if (ch === "h") out += "ℎ";
            else if (c >= 97 && c <= 122) out += String.fromCodePoint(0x1D44E + c - 97);
            else if (c >= 65 && c <= 90) out += String.fromCodePoint(0x1D434 + c - 65);
            else if (c >= 0x3B1 && c <= 0x3C9) out += String.fromCodePoint(0x1D6FC + c - 0x3B1);
            else out += ch;
        }
        return out;
    }
    function shift(p, dx, dy) {
        return p.map(q => {
            const r = Object.assign({}, q);
            r.x += dx; r.y += dy;
            if (r.k === "path") r.d = r.d.map(pt => [pt[0] + dx, pt[1] + dy, pt[2]]);
            return r;
        });
    }
    // returns {w, asc, desc, p: prims}, baseline at y = 0, x from 0
    function layout(n, px) {
        if (!n) return { w: 0, asc: 0, desc: 0, p: [] };
        const axis = px * 0.26, stroke = Math.max(1, px * 0.06);
        switch (n.k) {
        case "t": {
            const s = n.it ? italic(n.s) : n.s;
            const pad = n.op ? px * 0.24 : 0;
            const w = measure(s, px);
            return { w: w + 2 * pad, asc: px * 0.72, desc: px * 0.24, p: [{ k: "t", x: pad, y: 0, s: s, px: px }] };
        }
        case "row": {
            let x = 0, asc = 0, desc = 0, p = [];
            for (const c of (n.c || [])) {
                const l = layout(c, px);
                p = p.concat(shift(l.p, x, 0));
                x += l.w; asc = Math.max(asc, l.asc); desc = Math.max(desc, l.desc);
            }
            return { w: x, asc: asc, desc: desc, p: p };
        }
        case "frac": {
            const cp = Math.max(10, px * 0.84);
            const a = layout(n.n, cp), b = layout(n.d, cp);
            const gap = px * 0.16, w = Math.max(a.w, b.w) + px * 0.36;
            const ny = -(axis + stroke / 2 + gap + a.desc), dy = -axis + stroke / 2 + gap + b.asc;
            let p = shift(a.p, (w - a.w) / 2, ny).concat(shift(b.p, (w - b.w) / 2, dy));
            p.push({ k: "r", x: px * 0.08, y: -axis - stroke / 2, w: w - px * 0.16, h: stroke });
            return { w: w + px * 0.06, asc: -ny + a.asc, desc: dy + b.desc, p: p };
        }
        case "sup": case "sub": {
            const b = layout(n.b, px), e = layout(n.e, Math.max(9, px * 0.7));
            if (n.k === "sup") {
                const raise = Math.max(px * 0.42, b.asc - e.asc * 0.55);
                return { w: b.w + e.w + px * 0.05, asc: Math.max(b.asc, raise + e.asc), desc: Math.max(b.desc, e.desc - raise),
                         p: b.p.concat(shift(e.p, b.w + px * 0.04, -raise)) };
            }
            const lower = px * 0.22;
            return { w: b.w + e.w + px * 0.04, asc: Math.max(b.asc, e.asc - lower), desc: Math.max(b.desc, lower + e.desc),
                     p: b.p.concat(shift(e.p, b.w + px * 0.02, lower)) };
        }
        case "sqrt": {
            const a = layout(n.a, px);
            const gap = px * 0.12, top = -(a.asc + gap), sw = px * 0.62;
            let idx = null, lead = 0;
            if (n.i) { idx = layout({ k: "t", s: n.i }, Math.max(8, px * 0.5)); lead = Math.max(0, idx.w - px * 0.22); }
            const pathPts = [[lead, -axis * 0.9, 0], [lead + px * 0.14, -axis * 1.15, 1], [lead + px * 0.32, a.desc, 1],
                             [lead + sw - px * 0.04, top, 1], [lead + sw + a.w + px * 0.08, top, 1]];
            let p = [{ k: "path", x: 0, y: 0, d: pathPts, sw: stroke }].concat(shift(a.p, lead + sw, 0));
            if (idx) p = p.concat(shift(idx.p, 0, -axis * 1.3 - idx.desc - px * 0.05));
            return { w: lead + sw + a.w + px * 0.14, asc: a.asc + gap + stroke, desc: a.desc + px * 0.06, p: p };
        }
        case "par": {
            const a = layout(n.a, px);
            const asc = Math.max(a.asc, px * 0.72) + px * 0.08, desc = Math.max(a.desc, px * 0.24) + px * 0.08;
            const bw = Math.max(px * 0.32, (asc + desc) * 0.12);
            const p = bracket(n.o || "(", 0, -asc, desc, bw, stroke, false)
                .concat(shift(a.p, bw + px * 0.06, 0))
                .concat(bracket(n.c || ")", bw + px * 0.12 + a.w, -asc, desc, bw, stroke, true));
            return { w: 2 * bw + a.w + px * 0.12, asc: asc, desc: desc, p: p };
        }
        case "mat": {
            const cp = px * 0.94;
            const rows = n.r || [];
            const cells = rows.map(r => r.map(c => layout(c, cp)));
            const ncol = rows.length ? rows[0].length : 0;
            const colW = [];
            for (let j = 0; j < ncol; j++) colW.push(Math.max(px * 0.6, ...cells.map(r => r[j] ? r[j].w : 0)));
            const rowA = cells.map(r => Math.max(px * 0.7, ...r.map(c => c.asc)));
            const rowD = cells.map(r => Math.max(px * 0.24, ...r.map(c => c.desc)));
            const rg = px * 0.32, cg = px * 0.9;
            let H = 0;
            for (let i = 0; i < cells.length; i++) H += rowA[i] + rowD[i] + (i ? rg : 0);
            const top = -(H / 2 + axis), bw = Math.max(px * 0.3, H * 0.08), pad = px * 0.2;
            const hrows = n.hr || [];
            let p = [], y = top, W = 0;
            for (let j = 0; j < ncol; j++) W += colW[j] + (j ? cg : 0);
            const augX = [];
            for (let i = 0; i < cells.length; i++) {
                const base = y + rowA[i];
                if (hrows.indexOf(i) >= 0)
                    p.push({ k: "bg", x: bw + pad * 0.4, y: y - rg * 0.4, w: W + pad * 1.2, h: rowA[i] + rowD[i] + rg * 0.8, hl: "chg" });
                let x = bw + pad;
                for (let j = 0; j < ncol; j++) {
                    if (n.aug !== undefined && j === n.aug && i === 0) augX.push(x - cg / 2);
                    const c = cells[i][j];
                    p = p.concat(shift(c.p, x + (colW[j] - c.w) / 2, base));
                    x += colW[j] + cg;
                }
                y += rowA[i] + rowD[i] + rg;
            }
            const bot = top + H;
            for (const ax of augX) p.push({ k: "r", x: ax, y: top, w: stroke, h: H });
            const o = n.o || "(";
            const c = o === "(" ? ")" : o === "[" ? "]" : o;
            p = bracket(o, 0, top - px * 0.08, bot + px * 0.08, bw, stroke, false).concat(p)
                .concat(bracket(c, bw + pad * 2 + W, top - px * 0.08, bot + px * 0.08, bw, stroke, true));
            return { w: 2 * bw + 2 * pad + W, asc: -top + px * 0.1, desc: bot + px * 0.1, p: p };
        }
        case "hl": {
            const a = layout(n.a, px);
            return { w: a.w + px * 0.2, asc: a.asc + px * 0.06, desc: a.desc + px * 0.06,
                     p: [{ k: "bg", x: 0, y: -a.asc - px * 0.06, w: a.w + px * 0.2, h: a.asc + a.desc + px * 0.12, hl: n.h || "chg" }].concat(shift(a.p, px * 0.1, 0)) };
        }
        }
        return { w: 0, asc: 0, desc: 0, p: [] };
    }
    // a bracket of height top..bot at x (width bw); right = mirrored
    function bracket(kind, x, top, bot, bw, stroke, right) {
        const mid = (top + bot) / 2;
        if (kind === "|") return [{ k: "r", x: x + bw / 2, y: top, w: stroke, h: bot - top }];
        if (kind === "[" || kind === "]") {
            const xs = right ? x + bw * 0.7 : x + bw * 0.25, xe = right ? x + bw * 0.1 : x + bw * 0.85;
            return [{ k: "path", x: 0, y: 0, sw: stroke, d: [[xe, top, 0], [xs, top, 1], [xs, bot, 1], [xe, bot, 1]] }];
        }
        // ( ): a quadratic curve, the flag 2 marks a control point
        const xo = right ? x + bw * 0.15 : x + bw * 0.85, xi = right ? x + bw * 0.85 : x + bw * 0.15;
        return [{ k: "path", x: 0, y: 0, sw: stroke, d: [[xo, top, 0], [xi - (right ? -0 : 0), mid, 2], [xo, bot, 3]] }];
    }
    function svg(d) {
        let s = "";
        for (let i = 0; i < d.length; i++) {
            const q = d[i];
            if (q[2] === 0) s += "M " + q[0] + " " + q[1] + " ";
            else if (q[2] === 1) s += "L " + q[0] + " " + q[1] + " ";
            else if (q[2] === 2) { const e = d[i + 1]; s += "Q " + q[0] + " " + q[1] + " " + e[0] + " " + e[1] + " "; i++; }
        }
        return s;
    }

    Repeater {
        model: mv.lay.p.filter(q => q.k === "bg")
        Rectangle {
            x: modelData.x + 1; y: mv.base + modelData.y; width: modelData.w; height: modelData.h
            radius: Math.min(6, height / 3)
            color: Qt.rgba(mv.accent.r, mv.accent.g, mv.accent.b, modelData.hl === "bad" ? 0.0 : 0.20)
        }
    }
    Repeater {
        model: mv.lay.p.filter(q => q.k === "t")
        Text {
            x: modelData.x + 1
            y: mv.base + modelData.y - baselineOffset
            text: modelData.s
            color: mv.color
            font.family: mv.family
            font.pixelSize: Math.max(1, Math.round(modelData.px))
        }
    }
    Repeater {
        model: mv.lay.p.filter(q => q.k === "r")
        Rectangle { x: modelData.x + 1; y: mv.base + modelData.y; width: Math.max(1, modelData.w); height: Math.max(1, modelData.h); color: mv.color }
    }
    Repeater {
        model: mv.lay.p.filter(q => q.k === "path")
        Shape {
            x: 1; y: mv.base
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeColor: mv.color
                strokeWidth: modelData.sw
                fillColor: "transparent"
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                PathSvg { path: mv.svg(modelData.d) }
            }
        }
    }
}
