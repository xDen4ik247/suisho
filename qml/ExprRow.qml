import QtQuick

// One row of the expression list: a coloured index tab, the input, and its result
// (exact value, decimal approximation, matrix grid, graph/function badge or the error).
Item {
    id: row
    property var app
    property var theme
    property int index: 0
    property var r: ({})                 // the engine's row
    readonly property color tint: app.palette[(r.color || 0) % app.palette.length]
    readonly property bool hasOut: r.kind === "expr" || r.kind === "def" || r.kind === "rel" || r.kind === "error" || r.kind === "solve"
                                   || r.kind === "graph" || r.kind === "func"
    property alias input: field
    height: Math.max(56, col.implicitHeight + 18)

    signal edited(string text)
    signal enter()
    signal up()
    signal down()
    signal removeEmpty()

    Rectangle {
        anchors.fill: parent
        color: field.activeFocus ? theme.alpha(theme.text, 0.06) : "transparent"
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    // index tab
    Rectangle {
        id: tab
        width: 34
        anchors.left: parent.left; anchors.top: parent.top; anchors.bottom: parent.bottom
        color: field.activeFocus ? theme.alpha(row.tint, 0.22) : theme.alpha(theme.text, 0.03)
        Text {
            anchors.top: parent.top; anchors.topMargin: 8; anchors.horizontalCenter: parent.horizontalCenter
            text: row.index + 1
            color: theme.subtext1
            font.family: theme.font; font.pixelSize: 11
        }
        Rectangle {
            visible: row.r.kind === "graph" || row.r.kind === "func" || !!row.r.plots
            width: 14; height: 14; radius: 7
            anchors.horizontalCenter: parent.horizontalCenter; anchors.bottom: parent.bottom; anchors.bottomMargin: 10
            color: row.r.hidden ? "transparent" : row.tint
            border.width: 2; border.color: row.tint
        }
    }
    Rectangle {
        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
        height: 1
        color: theme.alpha(theme.text, 0.07)
    }

    Column {
        id: col
        anchors.left: tab.right; anchors.right: parent.right; anchors.top: parent.top; anchors.leftMargin: 14; anchors.rightMargin: 14; anchors.topMargin: 9
        spacing: 4

        // the row's input: typeset math while you're not editing it (click to edit), the text while you are
        Item {
            id: inputBox
            readonly property bool pretty: !field.activeFocus && !!row.r.in && row.r.kind !== "empty" && field.text !== ""
            width: parent.width
            height: pretty ? Math.max(28, prettyView.implicitHeight) : field.implicitHeight
            Flickable {
                anchors.fill: parent
                visible: inputBox.pretty
                contentWidth: prettyView.implicitWidth
                interactive: contentWidth > width
                clip: true
                MathView { id: prettyView; tree: row.r.in || null; size: 20; color: theme.text; accent: theme.accent; ru: app.ru; family: theme.mathOut }
                MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: { field.forceActiveFocus(); field.cursorPosition = field.text.length; } }
            }
        TextInput {
            id: field
            opacity: inputBox.pretty ? 0 : 1
            width: parent.width
            text: row.r.text || ""
            color: theme.text
            selectionColor: theme.alpha(theme.accent, 0.45)
            selectedTextColor: theme.text
            font.family: theme.mathInput; font.pixelSize: 19
            wrapMode: TextInput.WrapAnywhere
            clip: true
            Text {
                anchors.fill: parent
                visible: !field.text && !field.activeFocus && row.index === 0
                text: app.tr("type math: 1/3 + 1/6,  A = [[2,1],[1,3]],  det(A),  y = x^2", "пишите: 1/3 + 1/6,  A = [[2,1],[1,3]],  det(A),  y = x^2")
                color: theme.alpha(theme.subtext1, 0.7)
                font: field.font
                elide: Text.ElideRight
            }
            property bool fixing: false
            onActiveFocusChanged: if (activeFocus) app.selRow = row.r.id
            onTextEdited: {
                if (fixing) return;
                const fixed = app.autocorrect(text, cursorPosition);
                if (fixed.text !== text) {
                    fixing = true;
                    text = fixed.text;
                    cursorPosition = fixed.cursor;
                    fixing = false;
                }
                row.edited(text);
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { row.enter(); event.accepted = true; }
                else if (event.key === Qt.Key_Up) { row.up(); event.accepted = true; }
                else if (event.key === Qt.Key_Down) { row.down(); event.accepted = true; }
                else if (event.key === Qt.Key_Backspace && text === "") { row.removeEmpty(); event.accepted = true; }
            }
        }

        }

        // result line
        Row {
            visible: (row.hasOut && !(row.r.kind === "def" && row.r.vtype === "number" && row.r.value === bareValue())) || !!row.r.steps || !!row.r.lm || !!row.r.d3
            anchors.right: parent.right
            spacing: 10
            function bareValue() { return (row.r.text || "").replace(/^.*=\s*/, "") }

            Text {
                visible: row.r.kind === "error"
                text: row.r.error || ""
                color: theme.bad
                font.family: theme.font; font.pixelSize: 13
                width: Math.min(implicitWidth, col.width - 10)
                elide: Text.ElideRight
            }
            Text {
                visible: row.r.kind === "graph" || row.r.kind === "func" || !!row.r.plots
                text: row.r.kind === "func" ? app.tr("function of ", "функция от ") + (row.r.params || []).join(", ")
                      : row.r.conic ? (app.ru ? row.r.conic.ru : row.r.conic.en)
                      : app.tr("graph", "график")
                color: theme.alpha(row.tint, 0.95)
                font.family: theme.font; font.pixelSize: 13; font.italic: true
            }
            Text {
                visible: row.r.kind === "rel"
                text: row.r.value === "true" ? app.tr("✓ true", "✓ верно") : app.tr("✗ false", "✗ неверно")
                color: row.r.value === "true" ? theme.good : theme.bad
                font.family: theme.font; font.pixelSize: 15
            }
            Rectangle {
                visible: !!row.r.steps && row.r.kind !== "error"
                anchors.verticalCenter: parent.verticalCenter
                width: stepsText.implicitWidth + 18; height: 26; radius: 9
                color: stepsMouse.containsMouse ? theme.alpha(theme.accent, 0.30) : theme.alpha(theme.accent, 0.14)
                Text { id: stepsText; anchors.centerIn: parent; text: app.tr("steps", "решение"); color: theme.text; font.family: theme.font; font.pixelSize: 12; font.weight: Font.Medium }
                MouseArea { id: stepsMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: app.openSteps(row.r.id) }
            }
            Rectangle {
                visible: !!row.r.d3
                anchors.verticalCenter: parent.verticalCenter
                width: d3Text.implicitWidth + 18; height: 26; radius: 9
                color: d3Mouse.containsMouse ? theme.alpha(theme.accent, 0.35) : theme.alpha(theme.accent, 0.16)
                Text { id: d3Text; anchors.centerIn: parent; text: "3D"; color: theme.text; font.family: theme.font; font.pixelSize: 12; font.weight: Font.DemiBold }
                MouseArea { id: d3Mouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: app.view3d = true }
            }
            Rectangle {
                visible: !!row.r.lm
                anchors.verticalCenter: parent.verticalCenter
                width: lmText.implicitWidth + 18; height: 26; radius: 9
                color: lmMouse.containsMouse ? theme.alpha(theme.c_peach, 0.35) : theme.alpha(theme.c_peach, 0.16)
                Text { id: lmText; anchors.centerIn: parent; text: app.tr("show map", "показать"); color: theme.text; font.family: theme.font; font.pixelSize: 12; font.weight: Font.Medium }
                MouseArea { id: lmMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: app.showLinmap(row.r.lm) }
            }
            // value box (scaled down to fit when the answer is wider than the row)
            Item {
                id: valueWrap
                readonly property real fit: Math.min(1, Math.max(0.35, (col.width - 120) / Math.max(1, valueBox.width)))
                visible: valueBox.visible
                width: valueBox.width * fit
                height: valueBox.height * fit
                anchors.verticalCenter: parent.verticalCenter
            Rectangle {
                id: valueBox
                scale: valueWrap.fit
                transformOrigin: Item.TopLeft
                visible: (row.r.kind === "expr" || row.r.kind === "def" || row.r.kind === "solve") && row.r.value !== undefined
                width: valueRow.implicitWidth + 18
                height: valueRow.implicitHeight + 8
                radius: 8
                color: theme.alpha(theme.text, 0.06)
                Row {
                    id: valueRow
                    anchors.centerIn: parent
                    spacing: 10
                    Text {
                        visible: row.r.kind !== "solve"
                        text: "="
                        color: theme.text
                        font.family: theme.mathOut; font.pixelSize: 18
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    MathView {
                        visible: !!row.r.m
                        tree: row.r.m || null
                        size: 19
                        color: theme.text
                        accent: theme.accent
                        ru: app.ru
                        family: theme.mathOut
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Text {
                        visible: !row.r.m
                        text: app.pretty(row.r.value || "")
                        color: theme.text
                        font.family: theme.mathOut; font.pixelSize: 18
                        anchors.verticalCenter: parent.verticalCenter
                    }
                    Text {
                        visible: !!row.r.approx
                        text: "≈ " + app.pretty(row.r.approx || "")
                        color: theme.subtext1
                        font.family: theme.mathOut; font.pixelSize: 15
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }
            }
        }

        // why a graph row can't be drawn, and the Desmos-style fix: add a slider for the undefined letter
        Row {
            readonly property string perr: app.plotErrors[row.r.id] || ""
            readonly property var missing: {
                const m = (perr || row.r.error || "").match(/^(\S+) is not defined/);
                return m ? m[1] : (row.r.free && row.r.free.length ? row.r.free[0] : "");
            }
            visible: (row.r.kind === "graph" && perr !== "") || missing !== ""
            anchors.right: parent.right
            spacing: 10
            Text {
                visible: row.r.kind === "graph" && parent.perr !== ""
                text: parent.perr
                color: theme.bad; font.family: theme.font; font.pixelSize: 13
                anchors.verticalCenter: parent.verticalCenter
            }
            Rectangle {
                visible: parent.missing !== ""
                width: addTxt.implicitWidth + 18; height: 26; radius: 9
                color: addMouse.containsMouse ? theme.alpha(theme.accent, 0.30) : theme.alpha(theme.accent, 0.14)
                Text { id: addTxt; anchors.centerIn: parent; text: "+ " + app.tr("slider ", "ползунок ") + parent.parent.missing; color: theme.text; font.family: theme.font; font.pixelSize: 12 }
                MouseArea { id: addMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: app.send({ cmd: "row_insert", after: row.r.id, text: parent.parent.missing + " = 1" }) }
            }
        }

        // slider for "a = 2.5" rows
        Item {
            id: slider
            readonly property var m: (row.r.kind === "def" && row.r.vtype === "number") ? (row.r.text || "").match(/^\s*([^=\s]+)\s*=\s*(-?\d*\.?\d+)\s*$/) : null
            readonly property real value: m ? parseFloat(m[2]) : 0
            property real lo: Math.min(-10, Math.floor(value))
            property real hi: Math.max(10, Math.ceil(value))
            visible: !!m
            width: parent.width; height: 30
            function setValue(v) {
                const step = (hi - lo) / 200;
                v = Math.round(Math.max(lo, Math.min(hi, v)) / step) * step;
                const txt = slider.m[1] + " = " + String(Number(v.toFixed(4)));
                field.text = txt;
                row.edited(txt);
            }
            Text { id: loT; anchors.verticalCenter: parent.verticalCenter; text: app.num(slider.lo); color: theme.subtext1; font.family: theme.font; font.pixelSize: 11 }
            Rectangle {
                id: track
                anchors { left: loT.right; right: hiT.left; leftMargin: 10; rightMargin: 10; verticalCenter: parent.verticalCenter }
                height: 4; radius: 2
                color: theme.alpha(theme.text, 0.15)
                Rectangle { width: knob.x + knob.width / 2; height: parent.height; radius: 2; color: theme.alpha(row.tint, 0.8) }
                Rectangle {
                    id: knob
                    width: 16; height: 16; radius: 8
                    anchors.verticalCenter: parent.verticalCenter
                    x: (slider.value - slider.lo) / Math.max(1e-9, slider.hi - slider.lo) * track.width - width / 2
                    color: row.tint
                    border.width: 2; border.color: theme.alpha(theme.base, 0.8)
                }
                MouseArea {
                    anchors { fill: parent; margins: -10 }
                    onPressed: e => slider.setValue(slider.lo + (e.x - 10) / track.width * (slider.hi - slider.lo))
                    onPositionChanged: e => { if (pressed) slider.setValue(slider.lo + (e.x - 10) / track.width * (slider.hi - slider.lo)); }
                }
            }
            Text { id: hiT; anchors { right: play.left; rightMargin: 8; verticalCenter: parent.verticalCenter } text: app.num(slider.hi); color: theme.subtext1; font.family: theme.font; font.pixelSize: 11 }
            Rectangle {
                id: play
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                width: 26; height: 26; radius: 13
                color: anim.running ? theme.alpha(row.tint, 0.4) : theme.alpha(theme.text, 0.08)
                Text { anchors.centerIn: parent; text: anim.running ? "❚❚" : "▶"; color: theme.text; font.pixelSize: 10 }
                MouseArea { anchors.fill: parent; onClicked: anim.running = !anim.running }
            }
            Timer {
                id: anim
                interval: 40; repeat: true
                property real dir: 1
                onTriggered: {
                    let v = slider.value + dir * (slider.hi - slider.lo) / 200;
                    if (v >= slider.hi || v <= slider.lo) dir = -dir;
                    slider.setValue(v);
                }
            }
        }
    }
}
