import QtQuick
import QtQuick3D
import QtQuick3D.Helpers

// 3D view: surfaces z = f(x, y) (and planes, anything linear in z) from the engine's meshes, lit and tinted
// by height in the row's colour; drag to orbit, wheel to zoom. Axes: x red, y green, z blue (z is up).
Item {
    id: g3
    property var app
    property var theme
    property var surfaces: []
    property real range: 5
    property int lastRid: -1
    function request() { if (visible && app.ready) lastRid = app.send({ cmd: "mesh", r: range, n: 90 }); }
    function receive(m) { if (m.rid === lastRid) surfaces = m.surfaces || []; }
    onVisibleChanged: if (visible) request()

    function v3(a) { const out = []; for (let i = 0; i + 2 < a.length; i += 3) out.push(Qt.vector3d(a[i], a[i + 1], a[i + 2])); return out; }
    function colors(s) {
        const c = app.palette[(s.color || 0) % app.palette.length];
        const out = [];
        const span = Math.max(1e-9, s.zmax - s.zmin);
        for (const h of s.h) {
            const t = Math.max(0, Math.min(1, (h - s.zmin) / span));
            const k = 0.45 + 0.75 * t;
            out.push(Qt.vector4d(Math.min(1, c.r * k), Math.min(1, c.g * k), Math.min(1, c.b * k), 1));
        }
        return out;
    }

    View3D {
        id: view
        anchors.fill: parent
        environment: SceneEnvironment {
            clearColor: "transparent"
            backgroundMode: SceneEnvironment.Transparent
            antialiasingMode: SceneEnvironment.MSAA
            antialiasingQuality: SceneEnvironment.High
        }
        Node {
            id: origin
            eulerRotation: Qt.vector3d(-25, -35, 0)
            PerspectiveCamera { id: cam; position: Qt.vector3d(0, 0, 640); clipNear: 1; clipFar: 5000; fieldOfView: 40 }
        }
        DirectionalLight { eulerRotation: Qt.vector3d(-45, -30, 0); brightness: 1.1 }
        DirectionalLight { eulerRotation: Qt.vector3d(40, 150, 0); brightness: 0.35 }
        // axes
        Model { source: "#Cylinder"; scale: Qt.vector3d(0.012, 2.4, 0.012); eulerRotation.z: 90; materials: PrincipledMaterial { baseColor: g3.theme.c_red; lighting: PrincipledMaterial.NoLighting } }
        Model { source: "#Cylinder"; scale: Qt.vector3d(0.012, 2.4, 0.012); eulerRotation.x: 90; materials: PrincipledMaterial { baseColor: g3.theme.c_green; lighting: PrincipledMaterial.NoLighting } }
        Model { source: "#Cylinder"; scale: Qt.vector3d(0.012, 2.4, 0.012); materials: PrincipledMaterial { baseColor: g3.theme.c_blue; lighting: PrincipledMaterial.NoLighting } }
        // floor grid outline
        Model {
            source: "#Rectangle"; eulerRotation.x: -90; scale: Qt.vector3d(2.0, 2.0, 1)
            materials: PrincipledMaterial { baseColor: g3.theme.alpha(g3.theme.text, 0.06); alphaMode: PrincipledMaterial.Blend; cullMode: Material.NoCulling; lighting: PrincipledMaterial.NoLighting }
        }
        Repeater3D {
            model: g3.surfaces.filter(s => s.pos)
            Model {
                geometry: ProceduralMesh {
                    positions: g3.v3(modelData.pos)
                    normals: g3.v3(modelData.nrm)
                    colors: g3.colors(modelData)
                    indexes: modelData.idx
                }
                materials: PrincipledMaterial {
                    baseColor: "white"
                    vertexColorsEnabled: true
                    cullMode: Material.NoCulling
                    roughness: 0.55
                    metalness: 0.0
                    opacity: 0.94
                    alphaMode: PrincipledMaterial.Blend
                }
            }
        }
    }
    OrbitCameraController { anchors.fill: parent; origin: origin; camera: cam }

    Rectangle {
        anchors { left: parent.left; bottom: parent.bottom; margins: 12 }
        width: info3.implicitWidth + 20; height: info3.implicitHeight + 12; radius: 9
        color: g3.theme.alpha(g3.theme.base, 0.7)
        Text {
            id: info3
            anchors.centerIn: parent
            text: g3.app.tr("x, y ∈ [−", "x, y ∈ [−") + g3.app.num(g3.range) + "; " + g3.app.num(g3.range) + "]   " +
                  g3.app.tr("drag: rotate · wheel: zoom", "тянуть: вращать · колесо: масштаб") +
                  (g3.surfaces.some(s => s.error) ? "   ·   " + g3.surfaces.filter(s => s.error)[0].error : "")
            color: g3.theme.subtext0; font.family: g3.theme.font; font.pixelSize: 12
        }
    }
    Column {
        anchors { right: parent.right; top: parent.top; margins: 12 }
        spacing: 6
        Repeater {
            model: [{ s: "+", f: () => { g3.range = Math.max(0.5, g3.range / 1.5); g3.request(); } },
                    { s: "−", f: () => { g3.range = Math.min(1e4, g3.range * 1.5); g3.request(); } }]
            Rectangle {
                width: 34; height: 34; radius: 10
                color: zm3.containsMouse ? g3.theme.alpha(g3.theme.accent, 0.28) : g3.theme.alpha(g3.theme.base, 0.6)
                border.width: 1; border.color: g3.theme.alpha(g3.theme.text, 0.12)
                Text { anchors.centerIn: parent; text: modelData.s; color: g3.theme.text; font.pixelSize: 18 }
                MouseArea { id: zm3; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.f() }
            }
        }
    }
}
