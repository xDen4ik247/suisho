import QtQuick

// The step-by-step solution of one row, sliding in over the graph panel.
Glass {
    id: sp
    property var app
    property var sol: null
    property bool open: false
    tintAlpha: 0.86
    radius: 16
    visible: opacity > 0.01
    opacity: open ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 160 } }

    Rectangle { anchors.fill: parent; radius: sp.radius; color: sp.theme ? sp.theme.alpha(sp.theme.base, 0.88) : "black" }
    Item {
        anchors { fill: parent; margins: 18 }
        Row {
            id: head
            width: parent.width
            spacing: 10
            Text {
                text: sp.sol ? (app.ru ? sp.sol.ru : sp.sol.en) : ""
                color: sp.theme.text
                font.family: sp.theme.font; font.pixelSize: 20; font.weight: Font.DemiBold
            }
            Rectangle {
                visible: !!sp.sol
                anchors.verticalCenter: parent.verticalCenter
                width: badge.implicitWidth + 16; height: 22; radius: 11
                color: sp.sol && sp.sol.verified ? sp.theme.alpha(sp.theme.good, 0.22) : sp.theme.alpha(sp.theme.bad, 0.22)
                Text {
                    id: badge
                    anchors.centerIn: parent
                    text: sp.sol && sp.sol.verified ? app.tr("✓ checked", "✓ проверено") : app.tr("not checked", "не проверено")
                    color: sp.theme.text; font.family: sp.theme.font; font.pixelSize: 11
                }
            }
        }
        Row {
            anchors { right: parent.right; top: parent.top; rightMargin: 34 }
            spacing: 6
            Repeater {
                model: [{ s: app.tr("Copy LaTeX", "Копировать LaTeX"), f: "latex" }, { s: "Markdown", f: "md" }]
                Rectangle {
                    width: cl.implicitWidth + 16; height: 26; radius: 8
                    color: cm.containsMouse ? sp.theme.alpha(sp.theme.accent, 0.3) : sp.theme.alpha(sp.theme.text, 0.07)
                    Text { id: cl; anchors.centerIn: parent; text: modelData.s; color: sp.theme.text; font.family: sp.theme.font; font.pixelSize: 12 }
                    MouseArea { id: cm; anchors.fill: parent; hoverEnabled: true; onClicked: app.exportSteps(modelData.f) }
                }
            }
        }
        Text {
            anchors { right: parent.right; top: parent.top }
            text: "✕"; color: sp.theme.subtext1; font.pixelSize: 18
            MouseArea { anchors { fill: parent; margins: -8 } onClicked: sp.open = false }
        }
        Flickable {
            id: fl
            anchors { left: parent.left; right: parent.right; top: head.bottom; bottom: parent.bottom; topMargin: 14 }
            contentHeight: body.implicitHeight + 30
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: body
                width: fl.width - 8
                spacing: 16
                MathView { tree: sp.sol ? sp.sol.problem : null; size: 22; color: sp.theme.text; accent: sp.theme.accent; ru: app.ru }
                Repeater {
                    model: sp.sol ? sp.sol.steps : []
                    StepItem { width: body.width; app: sp.app; theme: sp.theme; s: modelData; num: String(index + 1) }
                }
                Rectangle { width: body.width; height: 1; color: sp.theme.alpha(sp.theme.text, 0.12) }
                Row {
                    spacing: 12
                    Text { text: app.tr("Answer", "Ответ"); color: sp.theme.subtext1; font.family: sp.theme.font; font.pixelSize: 15; anchors.verticalCenter: parent.verticalCenter }
                    MathView { tree: sp.sol ? sp.sol.answer : null; size: 22; color: sp.theme.text; accent: sp.theme.accent; ru: app.ru; anchors.verticalCenter: parent.verticalCenter }
                }
            }
        }
    }
}
