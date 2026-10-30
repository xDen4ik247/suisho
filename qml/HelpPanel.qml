import QtQuick

// What you can type (F1).
Glass {
    id: hp
    property var app
    property bool open: false
    tintAlpha: 0.92
    radius: 16
    visible: opacity > 0.01
    opacity: open ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 140 } }
    Rectangle { anchors.fill: parent; radius: hp.radius; color: hp.theme ? hp.theme.alpha(hp.theme.base, 0.92) : "black" }
    Text {
        anchors { right: parent.right; top: parent.top; margins: 18 }
        text: "✕"; color: hp.theme.subtext1; font.pixelSize: 18
        MouseArea { anchors { fill: parent; margins: -8 } onClicked: hp.open = false }
    }
    Flickable {
        anchors { fill: parent; margins: 20; topMargin: 18 }
        contentHeight: help.implicitHeight + 20
        clip: true
        Text {
            id: help
            width: parent.width - 10
            wrapMode: Text.WordWrap
            textFormat: Text.StyledText
            color: hp.theme.text
            font.family: hp.theme.font; font.pixelSize: 14
            lineHeight: 1.25
            text: hp.app.ru ? hp.ru : hp.en
        }
    }
    readonly property string en: "<h2>What you can type</h2>" +
        "<b>Numbers</b>: 1/3 + 1/6 · 2^100 · sqrt(8) = 2√2 · 5! · log_2 8 · 30° · 0b1011, 0x1F · |x − 1|<br>" +
        "<b>Definitions</b> (any order): a = 3 · f(x) = x^2 + a · f(2)<br>" +
        "<b>Graphs</b>: y = x^2 · x = y^2 · x^2 + y^2 = 9 · y &lt; sin x · r = 2cos(3θ) · (cos t, sin t) · (1, 2) · y = x^2 {x &gt; 0}<br>" +
        "<b>Sliders</b>: a row a = 2 gets one; an undefined letter offers “+ slider”.<br>" +
        "<b>Equations</b>: x^2 − 5x + 6 = 0 · x^3 − 6x^2 + 11x − 6 = 0 · x^2 − x − 6 &gt; 0 · solve(2x + y = 5, x − y = 1)<br>" +
        "<b>Algebra</b>: expand((x − 2)^3) · factor(x^3 − x)<br>" +
        "<b>Matrices</b>: A = [[2,1],[1,3]] or [2,1;1,3] · det(A) · A^−1 · inv(A) · A^T · rref(A) · rank(A) · eig(A) · <i>show map</i> on 2×2<br>" +
        "<b>Calculus</b>: diff(x^2 sin x) · integrate(x e^x) · integrate(sin x, 0, π) · lim(sin(x)/x, x, 0) · lim(f, x, inf) · analyze(f) · tangent(f, 1)<br>" +
        "<b>Geometry</b>: any 2nd-order curve (steps → canonical form) · line(P, Q) · distance(P, Q) · distance(P, 3x + y − 12 = 0) · midpoint · triangle(A, B, C) · plane(A, B, C) · dot · cross · angle<br>" +
        "<b>3D</b>: z = sin(x) cos(y) · x + y + z = 6 (the 3D switch)<br>" +
        "<b>Statistics</b>: mean, median, stdev, var, sort, corr, linreg([1,2,3],[2,4,7]) · normalcdf(a, b, μ, σ) · binompdf(n, p, k) · poissonpdf(λ, k)<br>" +
        "<b>Units</b>: 36 km/h to m/s · 100 °F to °C · 1 atm to Pa · c to km/s<br>" +
        "<b>Bases</b>: bin(10) · hex(255) · base(100, 3) · bitand, bitor, bitxor, shl, shr<br><br>" +
        "<b>Keys</b>: Enter new row · ↑↓ move · Backspace on an empty row deletes it · Ctrl+Z / Ctrl+Shift+Z undo/redo · Ctrl+N new sheet · Ctrl+O sheets · Ctrl+T trainer · Ctrl+L EN/RU · Ctrl+D RAD/DEG · F1 this help · Esc close panels. Typing sqrt, pi, theta, &lt;= turns into √, π, θ, ≤."
    readonly property string ru: "<h2>Что можно вводить</h2>" +
        "<b>Числа</b>: 1/3 + 1/6 · 2^100 · sqrt(8) = 2√2 · 5! · log_2 8 · 30° · 0b1011, 0x1F · |x − 1|<br>" +
        "<b>Определения</b> (в любом порядке): a = 3 · f(x) = x^2 + a · f(2)<br>" +
        "<b>Графики</b>: y = x^2 · x = y^2 · x^2 + y^2 = 9 · y &lt; sin x · r = 2cos(3θ) · (cos t, sin t) · (1, 2) · y = x^2 {x &gt; 0}<br>" +
        "<b>Ползунки</b>: строка a = 2 получает ползунок; неизвестная буква — кнопку «+ ползунок».<br>" +
        "<b>Уравнения</b>: x^2 − 5x + 6 = 0 · x^3 − 6x^2 + 11x − 6 = 0 · x^2 − x − 6 &gt; 0 · solve(2x + y = 5, x − y = 1)<br>" +
        "<b>Алгебра</b>: expand((x − 2)^3) · factor(x^3 − x)<br>" +
        "<b>Матрицы</b>: A = [[2,1],[1,3]] или [2,1;1,3] · det(A) · A^−1 · inv(A) · A^T · rref(A) · rank(A) · eig(A) · <i>показать</i> для 2×2<br>" +
        "<b>Анализ</b>: diff(x^2 sin x) · integrate(x e^x) · integrate(sin x, 0, π) · lim(sin(x)/x, x, 0) · lim(f, x, inf) · analyze(f) · tangent(f, 1)<br>" +
        "<b>Геометрия</b>: любая кривая 2-го порядка (решение → канонический вид) · line(P, Q) · distance(P, Q) · distance(P, 3x + y − 12 = 0) · midpoint · triangle(A, B, C) · plane(A, B, C) · dot · cross · angle<br>" +
        "<b>3D</b>: z = sin(x) cos(y) · x + y + z = 6 (переключатель 3D)<br>" +
        "<b>Статистика</b>: mean, median, stdev, var, sort, corr, linreg([1,2,3],[2,4,7]) · normalcdf(a, b, μ, σ) · binompdf(n, p, k) · poissonpdf(λ, k)<br>" +
        "<b>Единицы</b>: 36 km/h to m/s · 100 °F в °C · 1 atm to Pa · c to km/s<br>" +
        "<b>Системы счисления</b>: bin(10) · hex(255) · base(100, 3) · bitand, bitor, bitxor, shl, shr<br><br>" +
        "<b>Клавиши</b>: Enter — новая строка · ↑↓ — переход · Backspace в пустой строке — удалить · Ctrl+Z / Ctrl+Shift+Z — отмена/повтор · Ctrl+N — новый лист · Ctrl+O — листы · Ctrl+T — тренажёр · Ctrl+L — EN/RU · Ctrl+D — RAD/DEG · F1 — справка · Esc — закрыть панели. sqrt, pi, theta, &lt;= превращаются в √, π, θ, ≤."
}
