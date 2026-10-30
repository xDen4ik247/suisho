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
    readonly property bool hasOut: r.kind === "expr" || r.kind === "def" || r.kind === "rel" || r.kind === "error"
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
            visible: row.r.kind === "graph" || row.r.kind === "func"
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

        TextInput {
            id: field
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

        // result line
        Row {
            visible: (row.hasOut && !(row.r.kind === "def" && row.r.vtype === "number" && row.r.value === bareValue())) || !!row.r.steps
            anchors.right: parent.right
            spacing: 10
            function bareValue() { return (row.r.text || "").replace(/^.*=\s*/, "") }

            Text {
                visible: row.r.kind === "error"
                text: row.r.error || ""
                color: theme.bad
                font.family: theme.font; font.pixelSize: 13
                width: Math.min(implicitWidth, col.width)
                wrapMode: Text.WordWrap
            }
            Text {
                visible: row.r.kind === "graph" || row.r.kind === "func"
                text: row.r.kind === "func" ? app.tr("function of ", "функция от ") + (row.r.params || []).join(", ")
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
            // value box
            Rectangle {
                visible: (row.r.kind === "expr" || row.r.kind === "def") && row.r.value !== undefined
                width: valueRow.implicitWidth + 18
                height: valueRow.implicitHeight + 8
                radius: 8
                color: theme.alpha(theme.text, 0.06)
                Row {
                    id: valueRow
                    anchors.centerIn: parent
                    spacing: 10
                    Text {
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
}
