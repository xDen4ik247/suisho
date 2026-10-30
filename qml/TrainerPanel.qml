import QtQuick

// Trainer: a generated problem, your answer (any equivalent form counts), ✓/✗, the full solution on request,
// and the next one at the level that keeps you around 75 % right (per topic).
Glass {
    id: tp
    property var app
    property bool open: false
    property var problem: null
    property var result: null       // {correct, solution, rating} after an answer
    property var stats: []
    property int streak: 0
    property string topic: "all"
    property string invalid: ""
    tintAlpha: 0.9
    radius: 16
    visible: opacity > 0.01
    opacity: open ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 160 } }
    onOpenChanged: if (open) { if (!problem) next(); Qt.callLater(() => answer.forceActiveFocus()); }

    function answerText(t) { answer.text = t; submit(); }
    function next() { result = null; invalid = ""; answer.text = ""; app.send({ cmd: "tr_next", topic: topic }); Qt.callLater(() => answer.forceActiveFocus()); }
    function submit() {
        if (result) { next(); return; }
        if (!problem || answer.text.trim() === "") return;
        app.send({ cmd: "tr_answer", pid: problem.pid, text: answer.text });
    }
    function receive(m) {
        if (m.type === "tr_problem") { problem = m.problem; stats = m.stats; streak = m.streak; }
        else if (m.type === "tr_result") {
            if (m.invalid) { invalid = m.invalid; return; }
            invalid = ""; result = m; stats = m.stats; streak = m.streak;
        }
    }
    readonly property var topicInfo: stats.find(s => problem && s.id === problem.topic) || null

    Rectangle { anchors.fill: parent; radius: tp.radius; color: tp.theme ? tp.theme.alpha(tp.theme.base, 0.9) : "black" }
    Item {
        anchors { fill: parent; margins: 20 }
        Text {
            id: title
            text: app.tr("Trainer", "Тренажёр")
            color: tp.theme.text; font.family: tp.theme.font; font.pixelSize: 22; font.weight: Font.DemiBold
        }
        Text {
            anchors { right: parent.right; top: parent.top }
            text: "✕"; color: tp.theme.subtext1; font.pixelSize: 18
            MouseArea { anchors { fill: parent; margins: -8 } onClicked: tp.open = false }
        }
        // topics
        Flow {
            id: topics
            anchors { left: parent.left; right: parent.right; top: title.bottom; topMargin: 14 }
            spacing: 6
            Repeater {
                model: [{ id: "all", en: "Mixed", ru: "Вперемешку" }].concat(tp.stats)
                Rectangle {
                    width: tl.implicitWidth + 18; height: 28; radius: 9
                    color: tp.topic === modelData.id ? tp.theme.alpha(tp.theme.accent, 0.35) : (tm.containsMouse ? tp.theme.alpha(tp.theme.text, 0.10) : tp.theme.alpha(tp.theme.text, 0.05))
                    Text {
                        id: tl
                        anchors.centerIn: parent
                        text: (app.ru ? modelData.ru : modelData.en) + (modelData.level ? "  · L" + modelData.level : "")
                        color: tp.theme.text; font.family: tp.theme.font; font.pixelSize: 12
                    }
                    MouseArea { id: tm; anchors.fill: parent; hoverEnabled: true; onClicked: { tp.topic = modelData.id; tp.next(); } }
                }
            }
        }
        // the problem
        Column {
            anchors { left: parent.left; right: parent.right; top: topics.bottom; topMargin: 26 }
            spacing: 14
            Text {
                text: tp.problem ? (app.ru ? tp.problem.task.ru : tp.problem.task.en) + "   " + (tp.topicInfo ? (app.ru ? tp.topicInfo.ru : tp.topicInfo.en) + ", " + app.tr("level ", "уровень ") + tp.problem.level : "") : ""
                color: tp.theme.subtext0; font.family: tp.theme.font; font.pixelSize: 14
            }
            Flickable {
                width: parent.width; height: pm.implicitHeight; contentWidth: pm.implicitWidth; clip: true
                interactive: contentWidth > width
                MathView { id: pm; tree: tp.problem ? tp.problem.math : null; size: 28; color: tp.theme.text; accent: tp.theme.accent; ru: app.ru }
            }
            Text {
                text: tp.problem ? (app.ru ? tp.problem.hint.ru : tp.problem.hint.en) : ""
                color: tp.theme.subtext1; font.family: tp.theme.font; font.pixelSize: 12
            }
            Rectangle {
                width: parent.width; height: 52; radius: 12
                color: tp.theme.alpha(tp.theme.text, 0.06)
                border.width: 1.5
                border.color: tp.result ? (tp.result.correct ? tp.theme.good : tp.theme.bad) : tp.theme.alpha(tp.theme.accent, answer.activeFocus ? 0.7 : 0.25)
                TextInput {
                    id: answer
                    anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
                    verticalAlignment: TextInput.AlignVCenter
                    color: tp.theme.text
                    font.family: tp.theme.mathInput; font.pixelSize: 20
                    readOnly: !!tp.result
                    onTextEdited: {
                        const f = app.autocorrect(text, cursorPosition);
                        if (f.text !== text) { text = f.text; cursorPosition = f.cursor; }
                    }
                    Keys.onReturnPressed: tp.submit()
                    Keys.onEnterPressed: tp.submit()
                    Keys.onEscapePressed: tp.open = false
                    Text {
                        anchors.fill: parent; verticalAlignment: Text.AlignVCenter
                        visible: !answer.text && !tp.result
                        text: app.tr("your answer, then Enter", "ваш ответ, затем Enter")
                        color: tp.theme.alpha(tp.theme.subtext1, 0.7); font: answer.font
                    }
                }
            }
            Text {
                visible: tp.invalid !== ""
                text: tp.invalid
                color: tp.theme.bad; font.family: tp.theme.font; font.pixelSize: 13
            }
            Row {
                visible: !!tp.result
                spacing: 12
                Text {
                    text: tp.result && tp.result.correct ? app.tr("✓ Right!", "✓ Верно!") : app.tr("✗ Not quite.", "✗ Не совсем.")
                    color: tp.result && tp.result.correct ? tp.theme.good : tp.theme.bad
                    font.family: tp.theme.font; font.pixelSize: 20; font.weight: Font.DemiBold
                    anchors.verticalCenter: parent.verticalCenter
                }
                Text {
                    visible: !!(tp.result && tp.result.solution)
                    text: app.tr("Answer:", "Ответ:")
                    color: tp.theme.subtext1; font.family: tp.theme.font; font.pixelSize: 14
                    anchors.verticalCenter: parent.verticalCenter
                }
                MathView {
                    tree: tp.result && tp.result.solution ? tp.result.solution.answer : null
                    size: 18; color: tp.theme.text; accent: tp.theme.accent; ru: app.ru
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
            Row {
                spacing: 10
                Repeater {
                    model: [
                        { s: app.tr("Check  ⏎", "Проверить  ⏎"), on: !tp.result, f: () => tp.submit() },
                        { s: app.tr("Show solution", "Показать решение"), on: !!(tp.result && tp.result.solution), f: () => app.showSolution(tp.result.solution) },
                        { s: app.tr("Next  ⏎", "Дальше  ⏎"), on: !!tp.result, f: () => tp.next() },
                        { s: app.tr("Skip", "Пропустить"), on: !tp.result, f: () => tp.next() }
                    ]
                    Rectangle {
                        visible: modelData.on
                        width: bl.implicitWidth + 24; height: 34; radius: 10
                        color: bm.containsMouse ? tp.theme.alpha(tp.theme.accent, 0.35) : tp.theme.alpha(tp.theme.accent, 0.16)
                        Text { id: bl; anchors.centerIn: parent; text: modelData.s; color: tp.theme.text; font.family: tp.theme.font; font.pixelSize: 13; font.weight: Font.Medium }
                        MouseArea { id: bm; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.f() }
                    }
                }
            }
        }
        // footer: streak and this topic's record
        Text {
            anchors { left: parent.left; bottom: parent.bottom }
            text: (tp.streak > 0 ? "🔥 " + tp.streak + app.tr(" in a row", " подряд") + "    " : "") +
                  (tp.topicInfo ? app.tr("rating ", "рейтинг ") + tp.topicInfo.rating + "   " + tp.topicInfo.correct + "/" + tp.topicInfo.answered + app.tr(" right", " верно") : "")
            color: tp.theme.subtext1; font.family: tp.theme.font; font.pixelSize: 13
        }
    }
}
