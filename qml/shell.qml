// Suisho 水晶 — math workbench: exact calculator, step-by-step solutions, matrices, geometry and a grapher.
// A quickshell window (app-id suisho, set by the launcher through QS_APP_ID) plus the Rust engine
// (bin/suisho-engine) over JSON lines on stdin/stdout. Run: suisho
import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
    id: shell
    // delegates and components take these (a `theme: theme` binding inside them would bind to itself)
    readonly property var themeRef: theme
    readonly property var appRef: app

    // ------------------------------------------------------------------ theme (Matugen colours, live)
    QtObject {
        id: theme
        property color base: "#0d0e13"
        property color mantle: "#1b1b21"
        property color crust: "#121318"
        property color text: "#e3e1e9"
        property color subtext0: "#c6c5d0"
        property color subtext1: "#90909a"
        property color surface0: "#1f1f25"
        property color surface1: "#292a2f"
        property color surface2: "#34343a"
        property color accent: "#b9c3ff"
        property color accent2: "#e5bad8"
        property color deep: "#384379"
        property color bad: "#ffb4ab"
        property color good: Qt.hsla(0.40, 0.55, 0.72, 1)
        property color c_blue: "#8aadf4"
        property color c_peach: "#f5a97f"
        property color c_green: "#a6da95"
        property color c_red: "#ed8796"
        property color c_mauve: "#c6a0f6"
        property color c_teal: "#8bd5ca"
        property color c_yellow: "#eed49f"
        property color c_pink: "#f5bde6"
        property string font: "Google Sans"
        property string mathOut: "DejaVu Math TeX Gyre"
        property string mathInput: "Google Sans"
        property string icons: iconFont.status === FontLoader.Ready ? iconFont.name : font
        property int radius: 20
        function mix(a, b, t) { return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t) }
        function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
    }
    FontLoader { id: iconFont; source: "file:///usr/lib/kitty/fonts/SymbolsNerdFontMono-Regular.ttf" }
    FileView {
        path: (Quickshell.env("HOME") || "") + "/.local/state/serpantinum/qs_colors.json"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: {
            try {
                const c = JSON.parse(text());
                const set = (k, v) => { if (v) theme[k] = v; };
                set("base", c.base); set("mantle", c.mantle); set("crust", c.crust); set("text", c.text);
                set("subtext0", c.subtext0); set("subtext1", c.subtext1); set("surface0", c.surface0);
                set("surface1", c.surface1); set("surface2", c.surface2); set("accent", c.blue);
                set("accent2", c.peach); set("deep", c.sapphire); set("bad", c.red);
                set("c_blue", c.blue); set("c_peach", c.peach); set("c_green", c.green); set("c_red", c.red);
                set("c_mauve", c.mauve); set("c_teal", c.teal); set("c_yellow", c.yellow); set("c_pink", c.pink);
            } catch (e) {}
        }
    }

    // ------------------------------------------------------------------ engine
    property string appDir: Quickshell.env("SUISHO_APP") || (Quickshell.shellDir + "/..")
    Process {
        id: engine
        command: [shell.appDir + "/bin/suisho-engine"]
        workingDirectory: shell.appDir
        stdinEnabled: true
        running: true
        stdout: SplitParser { onRead: data => app.receive(data) }
        stderr: SplitParser { onRead: data => console.warn("engine:", data) }
        onExited: (code, status) => {
            app.ready = false;
            if (app.closing) return;
            // restart once a second at most; the workspace is autosaved, so nothing is lost
            app.restarts += 1;
            if (app.restarts > 5) { app.fatal = app.tr("The math engine keeps stopping (exit " + code + ").", "Движок всё время падает (код " + code + ")."); return; }
            restartTimer.start();
        }
        onStarted: app.send({ cmd: "hello" })
    }
    Timer { id: restartTimer; interval: 800; onTriggered: engine.running = true }

    // ------------------------------------------------------------------ state
    QtObject {
        id: app
        property var settings: ({ lang: "en", angle: "rad", digits: 10 })
        property var ws: ({ id: "", name: "" })
        property var workspaces: []
        property string fatal: ""
        property bool closing: false
        property bool ready: false            // the engine answered hello (requests before that would be lost)
        property int restarts: 0
        property int rid: 0
        property bool focusNext: false        // focus the row the next doc reply names (after Enter / ↓)
        property string toast: ""
        readonly property var palette: [theme.c_blue, theme.c_red, theme.c_green, theme.c_mauve, theme.c_peach, theme.c_teal, theme.c_yellow, theme.c_pink]
        readonly property bool ru: settings.lang === "ru"
        function tr(en, ruText) { return ru ? ruText : en }

        function send(obj) { rid += 1; obj.rid = rid; engine.write(JSON.stringify(obj) + "\n"); return rid; }
        function receive(line) {
            let m;
            try { m = JSON.parse(line); } catch (e) { console.warn("bad line from engine:", line); return; }
            if (m.type === "hello") {
                ready = true;
                settings = m.settings; workspaces = m.workspaces || []; applyDoc(m.doc); restarts = 0;
                graph.request();
            } else if (m.type === "doc") {
                applyDoc(m);
                if (m.workspaces) workspaces = m.workspaces;
                graph.request();
                if (graph3dLoader.item) graph3dLoader.item.request();
            } else if (m.type === "settings") {
                settings = m.settings; applyDoc(m.doc); graph.request();
            } else if (m.type === "plot") {
                graph.receive(m);
            } else if (m.type === "mesh") {
                if (graph3dLoader.item) graph3dLoader.item.receive(m);
            } else if (m.type === "tr_problem" || m.type === "tr_result" || m.type === "tr_stats") {
                trainerPanel.receive(m);
            } else if (m.type === "export") {
                Quickshell.clipboardText = m.text;
                showToast(m.fmt === "latex" ? tr("LaTeX copied", "LaTeX скопирован") : tr("Markdown copied", "Markdown скопирован"));
            } else if (m.type === "steps") {
                if (m.id === stepsFor) { stepsPanel.sol = m.sol; stepsPanel.open = true; }
            } else if (m.type === "ws_list") {
                workspaces = m.workspaces;
            } else if (m.type === "error") {
                showToast(m.error);
            }
        }
        function applyDoc(d) {
            if (!d) return;
            ws = d.ws || ws;
            const rows = d.rows || [];
            // update in place so the row being edited keeps its focus and cursor
            for (let i = 0; i < rows.length; i++) {
                const r = rows[i];
                let j = -1;
                for (let k = i; k < rowModel.count; k++) if (rowModel.get(k).rid === r.id) { j = k; break; }
                if (j < 0) rowModel.insert(i, { rid: r.id, rowData: r });
                else {
                    if (j > i) rowModel.move(j, i, 1);
                    rowModel.setProperty(i, "rowData", r);
                }
            }
            while (rowModel.count > rows.length) rowModel.remove(rowModel.count - 1);
            if (d.focus !== undefined && focusNext) { focusNext = false; Qt.callLater(() => focusRow(d.focus)); }
        }
        function focusRow(id) {
            for (let i = 0; i < rowModel.count; i++) if (rowModel.get(i).rid === id) {
                list.currentIndex = i;
                const it = list.itemAtIndex(i);
                if (it) { it.input.forceActiveFocus(); it.input.cursorPosition = it.input.text.length; }
                return;
            }
        }
        function focusIndex(i) {
            if (i < 0 || i >= rowModel.count) return;
            list.currentIndex = i;
            list.positionViewAtIndex(i, ListView.Contain);
            const it = list.itemAtIndex(i);
            if (it) it.input.forceActiveFocus();
        }
        function showSolution(sol, source) { stepsFor = null; stepsSource = source || ""; stepsPanel.sol = sol; stepsPanel.open = true; }
        property string stepsSource: ""
        // copy the open solution as LaTeX or Markdown (the engine writes it)
        function exportSteps(fmt) {
            if (stepsFor !== null) send({ cmd: "export", id: stepsFor, fmt: fmt });
            else if (stepsSource) send({ cmd: "export", source: stepsSource, fmt: fmt });
        }
        function showLinmap(m) { stepsPanel.open = false; graph.showLinmap(m); }
        function openSteps(id) { stepsFor = id; send({ cmd: "steps", id: id }); }
        property var stepsFor: null
        property bool view3d: false
        property var selRow: null           // the row being edited (its points of interest are shown)
        property var plotErrors: ({})       // row id → why it can't be graphed (e.g. "a is not defined")
        function num(v) {
            if (!isFinite(v)) return "—";
            const a = Math.abs(v);
            let s = a !== 0 && (a < 1e-4 || a >= 1e7) ? v.toExponential(4) : String(Number(v.toPrecision(7)));
            return s.replace("-", "−");
        }
        function setSetting(k, v) { const s = {}; s[k] = v; send({ cmd: "settings_set", set: s }); }
        function showToast(t) { toast = t; toastTimer.restart(); }
        function close() { closing = true; send({ cmd: "quit" }); Qt.callLater(Qt.quit); }

        // typing helpers: sqrt → √, pi → π, <= → ≤ … (the engine reads both forms)
        readonly property var fixes: [["sqrt", "√"], ["cbrt", "∛"], ["pi", "π"], ["theta", "θ"], ["alpha", "α"], ["beta", "β"],
                                      ["lambda", "λ"], ["phi", "φ"], ["infinity", "∞"], ["<=", "≤"], [">=", "≥"], ["!=", "≠"], ["*", "·"]]
        function autocorrect(text, cursor) {
            for (const [a, b] of fixes) {
                const at = cursor - a.length;
                if (at >= 0 && text.substr(at, a.length) === a) {
                    // only whole words for letter names (so "spin" stays)
                    if (/^[a-z]/.test(a) && at > 0 && /[a-z]/i.test(text[at - 1])) continue;
                    return { text: text.slice(0, at) + b + text.slice(cursor), cursor: at + b.length };
                }
            }
            return { text: text, cursor: cursor };
        }
        function pretty(s) { return String(s).replace(/-/g, "−").replace(/\*/g, "·") }
    }
    ListModel { id: rowModel }
    Timer { id: toastTimer; interval: 3500; onTriggered: app.toast = "" }

    // ------------------------------------------------------------------ window
    FloatingWindow {
        id: win
        title: Quickshell.env("SUISHO_TITLE") || "Suisho"
        implicitWidth: parseInt(Quickshell.env("SUISHO_W") || "1320")
        implicitHeight: parseInt(Quickshell.env("SUISHO_H") || "840")
        minimumSize: Qt.size(640, 480)
        color: "transparent"
        onClosed: app.close()

        Item {
            id: root
            anchors.fill: parent
            Rectangle {
                id: bg
                anchors.fill: parent
                radius: theme.radius
                gradient: Gradient {
                    GradientStop { position: 0.0; color: theme.alpha(theme.mix(theme.base, theme.deep, 0.30), 0.64) }
                    GradientStop { position: 1.0; color: theme.alpha(theme.base, 0.70) }
                }
            }
            focus: true
            Shortcut { sequences: ["Ctrl+N"]; onActivated: { wsPanel.open = false; app.send({ cmd: "ws_new" }); } }
            Shortcut { sequences: ["Ctrl+O"]; onActivated: { wsPanel.open = !wsPanel.open; if (wsPanel.open) app.send({ cmd: "ws_list" }); } }
            Shortcut { sequences: ["Ctrl+L"]; onActivated: app.setSetting("lang", app.ru ? "en" : "ru") }
            Shortcut { sequences: ["Ctrl+D"]; onActivated: app.setSetting("angle", app.settings.angle === "deg" ? "rad" : "deg") }
            Shortcut { sequences: ["Ctrl+Shift+Z"]; onActivated: app.send({ cmd: "redo" }) }
            Shortcut { sequences: ["Ctrl+T"]; onActivated: trainerPanel.open = !trainerPanel.open }
            Shortcut { sequences: ["F1"]; onActivated: helpPanel.open = !helpPanel.open }
            Shortcut { sequences: ["Escape"]; enabled: wsPanel.open || stepsPanel.open; onActivated: { if (wsPanel.open) wsPanel.open = false; else stepsPanel.open = false; } }

            // ---------------------------------------------------------- header
            Item {
                id: header
                anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 14
                height: 46
                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 12
                    Text { text: "水晶"; color: theme.accent; font.family: "Noto Sans CJK JP"; font.pixelSize: 24; font.weight: Font.Bold }
                    Text { text: "Suisho"; color: theme.text; font.family: theme.font; font.pixelSize: 22; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
                    Rectangle { width: 1; height: 24; color: theme.alpha(theme.text, 0.15); anchors.verticalCenter: parent.verticalCenter }
                    // workspace name (click: the workspace list)
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: wsName.implicitWidth + 28; height: 32; radius: 10
                        color: wsMouse.containsMouse || wsPanel.open ? theme.alpha(theme.text, 0.10) : theme.alpha(theme.text, 0.04)
                        Text { id: wsName; anchors.centerIn: parent; text: (app.ws.name || "Untitled") + "  ▾"; color: theme.subtext0; font.family: theme.font; font.pixelSize: 15 }
                        MouseArea { id: wsMouse; anchors.fill: parent; hoverEnabled: true; onClicked: { wsPanel.open = !wsPanel.open; if (wsPanel.open) app.send({ cmd: "ws_list" }); } }
                    }
                }
                Row {
                    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                    spacing: 8
                    Repeater {
                        model: [
                            { label: app.settings.angle === "deg" ? "DEG" : "RAD", tip: "Ctrl+D", act: () => app.setSetting("angle", app.settings.angle === "deg" ? "rad" : "deg") },
                            { label: app.ru ? "RU" : "EN", tip: "Ctrl+L", act: () => app.setSetting("lang", app.ru ? "en" : "ru") },
                            { label: "?", tip: "F1", act: () => helpPanel.open = !helpPanel.open },
                            { label: app.tr("Trainer", "Тренажёр"), tip: "Ctrl+T", act: () => trainerPanel.open = !trainerPanel.open },
                            { label: app.tr("New", "Новый"), tip: "Ctrl+N", act: () => app.send({ cmd: "ws_new" }) }
                        ]
                        Rectangle {
                            width: chipText.implicitWidth + 26; height: 32; radius: 10
                            color: chipMouse.containsMouse ? theme.alpha(theme.accent, 0.22) : theme.alpha(theme.text, 0.06)
                            Text { id: chipText; anchors.centerIn: parent; text: modelData.label; color: theme.text; font.family: theme.font; font.pixelSize: 14; font.weight: Font.Medium }
                            MouseArea { id: chipMouse; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.act() }
                        }
                    }
                }
            }

            // ---------------------------------------------------------- expression list
            Glass {
                id: listPanel
                theme: shell.themeRef
                anchors.left: parent.left; anchors.top: header.bottom; anchors.bottom: parent.bottom; anchors.margins: 14; anchors.topMargin: 8
                width: Math.max(380, Math.min(560, parent.width * 0.40))
                radius: 16
                ListView {
                    id: list
                    anchors.fill: parent; anchors.margins: 1
                    clip: true
                    model: rowModel
                    boundsBehavior: Flickable.StopAtBounds
                    delegate: ExprRow {
                        width: list.width
                        app: shell.appRef
                        theme: shell.themeRef
                        index: model.index
                        r: model.rowData || ({})
                        onEdited: text => app.send({ cmd: "row_set", id: model.rid, text: text })
                        onEnter: { app.focusNext = true; app.send({ cmd: "row_insert", after: model.rid }); }
                        onUp: app.focusIndex(model.index - 1)
                        onDown: {
                            if (model.index + 1 < rowModel.count) app.focusIndex(model.index + 1);
                            else if (input.text !== "") { app.focusNext = true; app.send({ cmd: "row_insert", after: model.rid }); }
                        }
                        onRemoveEmpty: {
                            if (rowModel.count <= 1) return;
                            const prev = Math.max(0, model.index - 1);
                            app.send({ cmd: "row_del", id: model.rid });
                            Qt.callLater(() => app.focusIndex(prev));
                        }
                    }
                    footer: Item {
                        width: list.width; height: 64
                        Text {
                            anchors.centerIn: parent
                            text: app.tr("Enter: new row · ↑↓ move · Ctrl+O workspaces · Ctrl+Z undo", "Enter: новая строка · ↑↓ переход · Ctrl+O листы · Ctrl+Z отмена")
                            color: theme.alpha(theme.subtext1, 0.8)
                            font.family: theme.font; font.pixelSize: 12
                        }
                        MouseArea { anchors.fill: parent; onClicked: { app.focusNext = true; app.send({ cmd: "row_insert" }); } }
                    }
                }
            }

            // ---------------------------------------------------------- graph panel (the grapher lands in M3)
            Glass {
                id: graphPanel
                theme: shell.themeRef
                anchors.left: listPanel.right; anchors.right: parent.right; anchors.top: header.bottom; anchors.bottom: parent.bottom; anchors.margins: 14; anchors.topMargin: 8; anchors.leftMargin: 10
                radius: 16
                tintAlpha: 0.22
                Graph2D {
                    id: graph
                    anchors { fill: parent; margins: 1 }
                    visible: !app.view3d
                    app: shell.appRef
                    theme: shell.themeRef
                }
                Loader {
                    id: graph3dLoader
                    anchors { fill: parent; margins: 1 }
                    active: app.view3d
                    visible: active
                    sourceComponent: Graph3D { app: shell.appRef; theme: shell.themeRef }
                }
                // 2D | 3D switch
                Row {
                    anchors { left: parent.left; top: parent.top; margins: 12 }
                    spacing: 0
                    z: 4
                    Repeater {
                        model: ["2D", "3D"]
                        Rectangle {
                            width: 44; height: 28
                            radius: 8
                            color: (index === 1) === app.view3d ? theme.alpha(theme.accent, 0.35) : theme.alpha(theme.base, 0.6)
                            border.width: 1; border.color: theme.alpha(theme.text, 0.12)
                            Text { anchors.centerIn: parent; text: modelData; color: theme.text; font.family: theme.font; font.pixelSize: 13; font.weight: Font.Medium }
                            MouseArea { anchors.fill: parent; onClicked: app.view3d = index === 1 }
                        }
                    }
                }
            }

            TrainerPanel {
                id: trainerPanel
                theme: shell.themeRef
                app: shell.appRef
                anchors.fill: graphPanel
                z: 4
            }

            HelpPanel {
                id: helpPanel
                theme: shell.themeRef
                app: shell.appRef
                anchors.fill: graphPanel
                z: 6
            }

            StepsPanel {
                id: stepsPanel
                theme: shell.themeRef
                app: shell.appRef
                anchors.fill: graphPanel
                z: 5
            }

            // ---------------------------------------------------------- workspace list
            Glass {
                id: wsPanel
                property bool open: false
                theme: shell.themeRef
                visible: opacity > 0
                opacity: open ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 140 } }
                x: header.x + 140; y: header.y + header.height + 2
                width: 360; height: Math.min(460, wsCol.implicitHeight + 24)
                radius: 14
                tintAlpha: 0.85
                z: 10
                Column {
                    id: wsCol
                    anchors.fill: parent; anchors.margins: 12
                    spacing: 4
                    Text { text: app.tr("Workspaces (autosaved)", "Листы (сохраняются сами)"); color: theme.subtext1; font.family: theme.font; font.pixelSize: 12 }
                    TextInput {
                        id: renameField
                        width: parent.width
                        text: app.ws.name || ""
                        color: theme.text
                        font.family: theme.font; font.pixelSize: 16; font.weight: Font.Medium
                        onAccepted: { app.send({ cmd: "ws_rename", name: text }); wsPanel.open = false; }
                        Rectangle { anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.bottom; height: 1; color: theme.alpha(theme.accent, renameField.activeFocus ? 0.8 : 0.25) }
                    }
                    Item { width: 1; height: 6 }
                    Repeater {
                        model: app.workspaces
                        Rectangle {
                            width: wsCol.width; height: 34; radius: 8
                            color: wsItemMouse.containsMouse ? theme.alpha(theme.accent, 0.18) : (modelData.id === app.ws.id ? theme.alpha(theme.text, 0.07) : "transparent")
                            Text {
                                anchors.left: parent.left; anchors.leftMargin: 10; anchors.verticalCenter: parent.verticalCenter
                                text: (modelData.name || "Untitled") + "   " + modelData.rows + app.tr(" rows", " строк")
                                color: theme.text; font.family: theme.font; font.pixelSize: 14
                            }
                            Text {
                                anchors.right: parent.right; anchors.rightMargin: 10; anchors.verticalCenter: parent.verticalCenter
                                text: "✕"; color: theme.subtext1; visible: wsItemMouse.containsMouse
                                font.pixelSize: 13
                                MouseArea { anchors.fill: parent; onClicked: app.send({ cmd: "ws_delete", ws: modelData.id }) }
                            }
                            MouseArea {
                                id: wsItemMouse
                                anchors.fill: parent; anchors.rightMargin: 30
                                hoverEnabled: true
                                onClicked: { app.send({ cmd: "ws_open", ws: modelData.id }); wsPanel.open = false; }
                            }
                        }
                    }
                }
            }

            // ---------------------------------------------------------- toast + fatal
            Rectangle {
                visible: app.toast !== ""
                anchors.horizontalCenter: parent.horizontalCenter; anchors.bottom: parent.bottom; anchors.bottomMargin: 26
                width: toastText.implicitWidth + 30; height: 38; radius: 12
                color: theme.alpha(theme.surface1, 0.95)
                z: 20
                Text { id: toastText; anchors.centerIn: parent; text: app.toast; color: theme.text; font.family: theme.font; font.pixelSize: 14 }
            }
            Text {
                visible: app.fatal !== ""
                anchors.centerIn: parent
                text: app.fatal
                color: theme.bad
                font.family: theme.font; font.pixelSize: 18
                z: 30
            }
        }
    }

    // ------------------------------------------------------------------ IPC (tests, screenshots)
    IpcHandler {
        target: "suisho"
        function shot(path: string): void { root.grabToImage(r => r.saveToFile(path)) }
        function state(): string { const out = []; for (let i = 0; i < rowModel.count; i++) out.push(rowModel.get(i).rowData); return JSON.stringify({ ws: app.ws, rows: out }) }
        function add(text: string): void { app.send({ cmd: "row_insert", text: text }) }
        function steps(i: int): void { if (i >= 0 && i < rowModel.count) app.openSteps(rowModel.get(i).rid) }
        function fresh(): void { app.send({ cmd: "ws_new", name: "Test" }) }
        function view(cx: real, cy: real, ppu: real): void { graph.cx = cx; graph.cy = cy; graph.ppu = ppu }
        function show3d(on: bool): void { app.view3d = on }
        function trainer(on: bool): void { trainerPanel.open = on }
        function trAnswer(text: string): void { trainerPanel.answerText(text) }
        function linmap(i: int, t: real): void { const r = rowModel.get(i).rowData; if (r && r.lm) { graph.showLinmap(r.lm); graph.lt = t; graph.stopAnim(); } }
    }
}
