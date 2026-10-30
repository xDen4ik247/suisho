import QtQuick
import QtQuick.Shapes

// The grapher: grid + axes + labels drawn here for the current view; curve geometry comes from the engine for
// the view it was sampled at (plus a margin) and is shown under a transform, so panning and zooming are
// instant (GPU) while a fresh, exact sample is fetched in the background (one request in flight).
Item {
    id: g
    property var app
    property var theme
    clip: true

    // ---- the view: centre and pixels per unit (equal aspect, like a math notebook)
    property real cx: 0
    property real cy: 0
    property real ppu: 50
    readonly property real x0: cx - width / 2 / ppu
    readonly property real x1: cx + width / 2 / ppu
    readonly property real y0: cy - height / 2 / ppu
    readonly property real y1: cy + height / 2 / ppu
    function sx(x) { return (x - x0) * ppu }
    function sy(y) { return (y1 - y) * ppu }

    // ---- what the engine sent: geometry in the pixel space of `sv`
    property var sv: null
    property var plotRows: []
    property var pois: []
    property bool inflight: false
    property bool dirty: false
    property int lastRid: -1
    property double sentAt: 0
    readonly property real k: sv ? (sv.x1 - sv.x0) / sv.w * ppu : 1
    readonly property real tx: sv ? (sv.x0 - x0) * ppu : 0
    readonly property real ty: sv ? (y1 - sv.y1) * ppu : 0

    function request() {
        if (width < 2 || height < 2 || !app.ready) return;
        if (inflight) { dirty = true; if (Date.now() - sentAt > 2500) inflight = false; else return; }
        sentAt = Date.now();
        inflight = true; dirty = false;
        lastRid = app.send({ cmd: "plot", view: { x0: x0, x1: x1, y0: y0, y1: y1, w: width, h: height } });
    }
    function receive(m) {
        if (m.rid !== lastRid) return;
        inflight = false;
        sv = m.view; plotRows = m.rows || []; pois = m.pois || [];
        const errs = {};
        for (const r of (m.rows || [])) if (r.error) errs[r.id] = r.error;
        app.plotErrors = errs;
        if (dirty) request();
    }
    // geometry is good for ±1 view of panning and moderate zoom; beyond that (or after a pause) fetch anew
    function viewChanged() {
        if (!sv) { request(); return; }
        const ratio = k;
        const far = Math.abs(tx) > width * 0.6 || Math.abs(ty) > height * 0.6;
        if (far || ratio < 0.8 || ratio > 1.25) request();
        settle.restart();
    }
    onCxChanged: viewChanged()
    onCyChanged: viewChanged()
    onPpuChanged: viewChanged()
    onWidthChanged: request()
    onHeightChanged: request()
    Timer { id: settle; interval: 90; onTriggered: g.request() }

    function zoomAt(px, py, f) {
        const mx = x0 + px / ppu, my = y1 - py / ppu;
        const np = Math.max(1e-6, Math.min(1e9, ppu * f));
        cx = mx - (px - width / 2) / np;
        cy = my + (py - height / 2) / np;
        ppu = np;
    }
    function home() { cx = 0; cy = 0; ppu = Math.min(width, height) / 20; }
    Component.onCompleted: { ppu = Math.max(20, Math.min(width, height) / 20); request(); }

    // ---- grid
    function niceStep(raw) {
        const p = Math.pow(10, Math.floor(Math.log10(raw)));
        const m = raw / p;
        return (m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10) * p;
    }
    readonly property real step: niceStep(110 / ppu)
    readonly property real minor: step / ((step / Math.pow(10, Math.floor(Math.log10(step)))) === 2 ? 4 : 5)
    function fmt(v) {
        if (Math.abs(v) < step * 1e-6) return "0";
        const d = Math.max(0, -Math.floor(Math.log10(step)));
        let s = Math.abs(v) >= 1e6 || Math.abs(v) < 1e-4 ? v.toExponential(1) : v.toFixed(d);
        return s.replace("-", "−");
    }
    Repeater {
        model: Math.max(0, Math.min(400, Math.ceil(g.width / (g.minor * g.ppu)) + 2))
        Rectangle {
            readonly property real v: (Math.floor(g.x0 / g.minor) + index) * g.minor
            readonly property bool major: Math.abs(v / g.step - Math.round(v / g.step)) < 1e-6
            x: g.sx(v); width: 1; height: g.height
            color: Math.abs(v) < g.minor * 1e-6 ? "transparent" : g.theme.alpha(g.theme.text, major ? 0.12 : 0.045)
        }
    }
    Repeater {
        model: Math.max(0, Math.min(400, Math.ceil(g.height / (g.minor * g.ppu)) + 2))
        Rectangle {
            readonly property real v: (Math.floor(g.y0 / g.minor) + index) * g.minor
            readonly property bool major: Math.abs(v / g.step - Math.round(v / g.step)) < 1e-6
            y: g.sy(v); height: 1; width: g.width
            color: Math.abs(v) < g.minor * 1e-6 ? "transparent" : g.theme.alpha(g.theme.text, major ? 0.12 : 0.045)
        }
    }
    // axes
    Rectangle { x: g.sx(0) - 0.75; width: 1.5; height: g.height; color: g.theme.alpha(g.theme.text, 0.6); visible: x > -2 && x < g.width + 2 }
    Rectangle { y: g.sy(0) - 0.75; height: 1.5; width: g.width; color: g.theme.alpha(g.theme.text, 0.6); visible: y > -2 && y < g.height + 2 }
    // labels (stick to the edge when the axis is off screen)
    readonly property real axisY: Math.max(4, Math.min(g.height - 20, g.sy(0) + 4))
    readonly property real axisX: Math.max(4, Math.min(g.width - 8, g.sx(0) - 6))
    Repeater {
        model: Math.max(0, Math.min(200, Math.ceil(g.width / (g.step * g.ppu)) + 2))
        Text {
            readonly property real v: (Math.floor(g.x0 / g.step) + index) * g.step
            visible: Math.abs(v) > g.step * 1e-6
            x: g.sx(v) - width / 2; y: g.axisY
            text: g.fmt(v); color: g.theme.subtext1
            font.family: g.theme.font; font.pixelSize: 12
        }
    }
    Repeater {
        model: Math.max(0, Math.min(200, Math.ceil(g.height / (g.step * g.ppu)) + 2))
        Text {
            readonly property real v: (Math.floor(g.y0 / g.step) + index) * g.step
            visible: Math.abs(v) > g.step * 1e-6
            x: g.axisX - width; y: g.sy(v) - height / 2
            text: g.fmt(v); color: g.theme.subtext1
            font.family: g.theme.font; font.pixelSize: 12
        }
    }

    // ---- curves (engine geometry under the view transform)
    Item {
        id: layer
        width: g.width; height: g.height
        transform: [Scale { xScale: g.k; yScale: g.k }, Translate { x: g.tx; y: g.ty }]
        Repeater {
            model: g.plotRows
            Item {
                readonly property color col: g.app.palette[(modelData.color || 0) % g.app.palette.length]
                Shape {
                    visible: !!modelData.fill
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath {
                        strokeWidth: -1
                        strokeColor: "transparent"
                        fillColor: Qt.rgba(col.r, col.g, col.b, 0.22)
                        fillRule: ShapePath.WindingFill
                        PathSvg { path: modelData.fill || "" }
                    }
                }
                Shape {
                    visible: !!modelData.path
                    preferredRendererType: Shape.CurveRenderer
                    ShapePath {
                        strokeColor: col
                        strokeWidth: 2.6
                        cosmeticStroke: true
                        fillColor: "transparent"
                        capStyle: ShapePath.RoundCap
                        joinStyle: ShapePath.RoundJoin
                        strokeStyle: modelData.dashed ? ShapePath.DashLine : ShapePath.SolidLine
                        dashPattern: [4, 3]
                        PathSvg { path: modelData.path || "" }
                    }
                }
            }
        }
    }
    // points (in current pixels, so they keep their size)
    Repeater {
        model: g.plotRows.filter(r => r.points)
        Item {
            readonly property var row: modelData
            readonly property color col: g.app.palette[(row.color || 0) % g.app.palette.length]
            Repeater {
                model: row.points
                Rectangle {
                    width: 11; height: 11; radius: 5.5
                    x: g.sx(modelData[2]) - 5.5; y: g.sy(modelData[3]) - 5.5
                    color: col
                    border.width: 2; border.color: g.theme.alpha(g.theme.base, 0.8)
                }
            }
        }
    }
    // points of interest: grey dots; hover or click shows the coordinates
    property var pinned: null
    Repeater {
        model: g.pois
        Rectangle {
            readonly property bool inView: g.sx(modelData.x) > -10 && g.sx(modelData.x) < g.width + 10 && g.sy(modelData.y) > -10 && g.sy(modelData.y) < g.height + 10
            // like Desmos: the points of interest of the row being edited (intersections: of either curve)
            visible: inView && (modelData.row === g.app.selRow || modelData.row2 === g.app.selRow || g.pinned === modelData)
            width: poiMouse.containsMouse || g.pinned === modelData ? 12 : 8; height: width; radius: width / 2
            x: g.sx(modelData.x) - width / 2; y: g.sy(modelData.y) - height / 2
            color: g.theme.alpha(g.theme.subtext0, poiMouse.containsMouse ? 0.95 : 0.65)
            border.width: 1.5; border.color: g.theme.alpha(g.theme.base, 0.9)
            MouseArea {
                id: poiMouse
                anchors { fill: parent; margins: -6 }
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: g.pinned = g.pinned === modelData ? null : modelData
            }
            Rectangle {
                visible: poiMouse.containsMouse || g.pinned === modelData
                x: 14; y: -30
                width: poiLabel.implicitWidth + 16; height: 26; radius: 8
                color: g.theme.alpha(g.theme.surface1, 0.95)
                Text {
                    id: poiLabel
                    anchors.centerIn: parent
                    text: "(" + g.app.num(modelData.x) + ", " + g.app.num(modelData.y) + ")" +
                          ({ root: g.app.tr("  zero", "  нуль"), max: g.app.tr("  max", "  максимум"), min: g.app.tr("  min", "  минимум"),
                             yint: g.app.tr("  y-intercept", "  пересечение с Oy"), cross: g.app.tr("  intersection", "  пересечение") }[modelData.kind] || "")
                    color: g.theme.text; font.family: g.theme.font; font.pixelSize: 13
                }
            }
        }
    }

    // ---- input: drag to pan, wheel/pinch to zoom
    MouseArea {
        id: mouse
        anchors.fill: parent
        z: -1
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        property real lx: 0
        property real ly: 0
        onPressed: e => { lx = e.x; ly = e.y; g.pinned = null; }
        onPositionChanged: e => {
            if (pressed) { g.cx -= (e.x - lx) / g.ppu; g.cy += (e.y - ly) / g.ppu; lx = e.x; ly = e.y; }
        }
        onWheel: e => {
            if (e.pixelDelta.y !== 0 && !(e.modifiers & Qt.ControlModifier) && Math.abs(e.angleDelta.y) < 120 && e.pixelDelta.x !== 0) {
                // touchpad two-finger scroll pans
                g.cx -= e.pixelDelta.x / g.ppu; g.cy += e.pixelDelta.y / g.ppu;
            } else {
                g.zoomAt(e.x, e.y, Math.pow(1.0018, e.angleDelta.y));
            }
        }
    }
    PinchHandler {
        target: null
        property real last: 1
        onActiveChanged: last = 1
        onScaleChanged: { g.zoomAt(centroid.position.x, centroid.position.y, activeScale / last); last = activeScale; }
    }

    // ---- corner: cursor position, zoom buttons, home
    Text {
        anchors { left: parent.left; bottom: parent.bottom; margins: 10 }
        visible: mouse.containsMouse
        text: "(" + g.app.num(g.x0 + mouse.mouseX / g.ppu) + ", " + g.app.num(g.y1 - mouse.mouseY / g.ppu) + ")"
        color: g.theme.subtext1; font.family: g.theme.font; font.pixelSize: 12
    }
    Column {
        anchors { right: parent.right; top: parent.top; margins: 12 }
        spacing: 6
        Repeater {
            model: [{ s: "+", f: () => g.zoomAt(g.width / 2, g.height / 2, 1.6) }, { s: "−", f: () => g.zoomAt(g.width / 2, g.height / 2, 1 / 1.6) },
                    { s: "⌂", f: () => g.home() }]
            Rectangle {
                width: 34; height: 34; radius: 10
                color: zm.containsMouse ? g.theme.alpha(g.theme.accent, 0.28) : g.theme.alpha(g.theme.base, 0.6)
                border.width: 1; border.color: g.theme.alpha(g.theme.text, 0.12)
                Text { anchors.centerIn: parent; text: modelData.s; color: g.theme.text; font.pixelSize: 18; font.family: g.theme.font }
                MouseArea { id: zm; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.f() }
            }
        }
    }
}
