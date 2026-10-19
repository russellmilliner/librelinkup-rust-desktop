import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Window {
    id: overlayWindow
    objectName: "librelinkupOverlayWindow"
    title: qsTr("LibreLinkUp Rust Desktop Overlay")
    width: 268
    height: 96
    minimumWidth: 220
    minimumHeight: 80
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    visible: Boolean(controller && controller.overlayVisible)

    property var controller: null
    readonly property bool isDarkOverlay: Boolean(controller ? controller.overlayDarkTheme : true)
    readonly property int opacityPercent: controller ? Math.max(20, Math.min(100, controller.overlayOpacityPercent)) : 88
    readonly property real opacityAlpha: opacityPercent / 100.0
    readonly property color statusAccentColor: Qt.color(controller ? controller.rangeColorHex : "#22c55e")

    Rectangle {
        id: overlayPill
        objectName: "overlayPill"
        anchors.fill: parent
        radius: 14
        color: overlayWindow.isDarkOverlay
            ? Qt.rgba(0.06, 0.09, 0.16, overlayWindow.opacityAlpha)
            : Qt.rgba(
                overlayWindow.statusAccentColor.r,
                overlayWindow.statusAccentColor.g,
                overlayWindow.statusAccentColor.b,
                overlayWindow.opacityAlpha
            )
        border.color: overlayWindow.isDarkOverlay
            ? Qt.rgba(
                overlayWindow.statusAccentColor.r,
                overlayWindow.statusAccentColor.g,
                overlayWindow.statusAccentColor.b,
                Math.min(1.0, overlayWindow.opacityAlpha + 0.15)
            )
            : Qt.rgba(1, 1, 1, 0.42)
        border.width: overlayWindow.isDarkOverlay ? 2.0 : 1.5

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onPressed: overlayWindow.startSystemMove()
            onDoubleClicked: {
                if (overlayWindow.controller) {
                    overlayWindow.controller.showMainWindow()
                }
            }
            onWheel: function(wheel) {
                if (overlayWindow.controller) {
                    const step = wheel.angleDelta.y > 0 ? 5 : -5
                    const nextVal = Math.max(20, Math.min(100, overlayWindow.opacityPercent + step))
                    overlayWindow.controller.updateOverlayOpacityPercent(nextVal)
                }
            }
        }

        RowLayout {
            objectName: "overlayRow"
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 8
            anchors.topMargin: 6
            anchors.bottomMargin: 6
            spacing: 6

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Rectangle {
                        visible: overlayWindow.isDarkOverlay
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        radius: 4
                        color: overlayWindow.statusAccentColor
                    }

                    QQC2.Label {
                        text: controller ? controller.glucoseDisplay : "--"
                        font.pointSize: 20
                        font.bold: true
                        color: "#ffffff"
                    }

                    QQC2.Label {
                        text: controller ? controller.resultUnit : "mg/dL"
                        font.pointSize: 9.5
                        font.bold: true
                        color: overlayWindow.isDarkOverlay
                            ? overlayWindow.statusAccentColor
                            : Qt.rgba(1, 1, 1, 0.92)
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        elide: Text.ElideRight
                    }
                }

                QQC2.Label {
                    text: controller
                        ? (controller.rangeStatusLabel + " • " + controller.trendDescription)
                        : qsTr("Waiting...")
                    font.pointSize: 8.5
                    color: overlayWindow.isDarkOverlay
                        ? Qt.rgba(0.88, 0.92, 0.98, 0.92)
                        : Qt.rgba(1, 1, 1, 0.90)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }
            }

            Rectangle {
                Layout.preferredWidth: 36
                Layout.preferredHeight: 36
                radius: 18
                color: overlayWindow.isDarkOverlay
                    ? Qt.rgba(
                        overlayWindow.statusAccentColor.r,
                        overlayWindow.statusAccentColor.g,
                        overlayWindow.statusAccentColor.b,
                        0.24
                    )
                    : Qt.rgba(1, 1, 1, 0.24)
                border.color: overlayWindow.isDarkOverlay
                    ? overlayWindow.statusAccentColor
                    : Qt.rgba(1, 1, 1, 0.45)
                border.width: 1.5

                QQC2.Label {
                    anchors.centerIn: parent
                    text: controller ? controller.trendSymbol : "→"
                    font.pointSize: 15
                    font.bold: true
                    color: "#ffffff"
                }
            }

            ColumnLayout {
                spacing: 1

                QQC2.ToolButton {
                    objectName: "overlayThemeToggleBtn"
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: overlayWindow.isDarkOverlay ? "weather-clear-night" : "color-picker"
                    implicitWidth: 22
                    implicitHeight: 22
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: overlayWindow.isDarkOverlay
                        ? qsTr("Overlay Dark Theme ON (Click for Color Fill • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                        : qsTr("Overlay Color Fill ON (Click for Dark Theme • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                    onClicked: {
                        if (overlayWindow.controller) {
                            overlayWindow.controller.updateOverlayDarkTheme(!overlayWindow.isDarkOverlay)
                        }
                    }
                }

                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "window-restore"
                    implicitWidth: 22
                    implicitHeight: 22
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: qsTr("Open LibreLinkUp Rust Desktop")
                    onClicked: {
                        if (overlayWindow.controller) {
                            overlayWindow.controller.showMainWindow()
                        }
                    }
                }

                QQC2.ToolButton {
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "window-close"
                    implicitWidth: 22
                    implicitHeight: 22
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: qsTr("Hide Overlay")
                    onClicked: {
                        if (overlayWindow.controller) {
                            overlayWindow.controller.updateOverlayVisible(false)
                        }
                    }
                }
            }
        }
    }
}
