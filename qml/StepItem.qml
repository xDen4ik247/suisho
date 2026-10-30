import QtQuick

// One step of a solution: its words (EN/RU), an optional math title (a row operation), its math lines,
// and its sub-steps (collapsible, e.g. the minors of a cofactor expansion).
Column {
    id: si
    property var app
    property var theme
    property var s: ({})
    property int depth: 0
    property string num: ""
    property bool expanded: depth < 1
    spacing: 8
    width: parent ? parent.width : 400

    Row {
        spacing: 10
        width: si.width
        Rectangle {
            visible: si.num !== ""
            width: 26; height: 26; radius: 13
            color: theme.alpha(theme.accent, si.depth ? 0.12 : 0.22)
            Text { anchors.centerIn: parent; text: si.num; color: theme.text; font.family: theme.font; font.pixelSize: 12; font.weight: Font.DemiBold }
        }
        Column {
            width: si.width - 40
            spacing: 6
            Text {
                width: parent.width
                text: app.ru ? (si.s.ru || "") : (si.s.en || "")
                color: si.depth ? theme.subtext0 : theme.text
                wrapMode: Text.WordWrap
                font.family: theme.font; font.pixelSize: si.depth ? 13 : 14
            }
            MathView {
                visible: !!si.s.head
                tree: si.s.head || null
                size: 16; color: theme.accent; accent: theme.accent; ru: app.ru
            }
            Repeater {
                model: si.s.math || []
                Flickable {
                    width: Math.min(parent.width, mvLine.implicitWidth)
                    height: mvLine.implicitHeight
                    contentWidth: mvLine.implicitWidth
                    clip: true
                    interactive: contentWidth > width
                    MathView { id: mvLine; tree: modelData; size: si.depth ? 16 : 18; color: theme.text; accent: theme.accent; ru: app.ru }
                }
            }
            // sub-steps
            Text {
                visible: (si.s.sub || []).length > 0
                text: (si.expanded ? "▾ " : "▸ ") + app.tr("details", "подробнее") + " (" + (si.s.sub || []).length + ")"
                color: theme.accent
                font.family: theme.font; font.pixelSize: 12
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: si.expanded = !si.expanded }
            }
            Column {
                visible: si.expanded && (si.s.sub || []).length > 0
                width: parent.width
                spacing: 10
                leftPadding: 12
                Repeater {
                    model: si.expanded ? (si.s.sub || []) : []
                    Loader {
                        width: parent.width - 12
                        source: "StepItem.qml"
                        onLoaded: { item.app = si.app; item.theme = si.theme; item.depth = si.depth + 1; item.s = modelData; item.num = ""; item.width = width; }
                    }
                }
            }
        }
    }
}
