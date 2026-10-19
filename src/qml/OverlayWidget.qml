import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Window {
    id: overlayWindow
    objectName: "librelinkupOverlayWindow"
    title: qsTr("LibreLinkUp Overlay")
    width: 248
    height: 92
    minimumWidth: 190
    minimumHeight: 72
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    visible: Boolean(controller && controller.overlayVisible)

    property var controller: null
    readonly property bool isTransparentMode: Boolean(
        controller && controller.windowMode === "overlayTransparent"
    )

    Rectangle {
        anchors.fill: parent
        radius: 14
        color: overlayWindow.isTransparentMode
            ? Qt.rgba(0.08, 0.10, 0.14, 0.52)
            : (controller ? controller.rangeColorHex : "#22c55e")
        border.color: Qt.rgba(1, 1, 1, 0.35)
        border.width: 1.5

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onPressed: overlayWindow.startSystemMove()
            onDoubleClicked: {
                if (overlayWindow.controller) {
                    overlayWindow.controller.showMainWindow()
                }
            }
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 12
            spacing: 10

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    spacing: 6
                    QQC2.Label {
                        text: controller ? controller.glucoseDisplay : "--"
                        font.pointSize: 22
                        font.bold: true
                        color: "#ffffff"
                    }
                    QQC2.Label {
                        text: controller ? controller.resultUnit : "mg/dL"
                        font.pointSize: 10
                        font.bold: true
                        color: Qt.rgba(1, 1, 1, 0.9)
                    }
                }

                QQC2.Label {
                    text: controller
                        ? (controller.rangeStatusLabel + " • " + controller.trendDescription)
                        : qsTr("Waiting...")
                    font.pointSize: 8.5
                    color: Qt.rgba(1, 1, 1, 0.88)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            Rectangle {
                width: 38
                height: 38
                radius: 19
                color: Qt.rgba(1, 1, 1, 0.24)
                border.color: Qt.rgba(1, 1, 1, 0.45)
                border.width: 1

                QQC2.Label {
                    anchors.centerIn: parent
                    text: controller ? controller.trendSymbol : "→"
                    font.pointSize: 16
                    font.bold: true
                    color: "#ffffff"
                }
            }

            ColumnLayout {
                spacing: 2
                QQC2.ToolButton {
                    icon.name: "window-restore"
                    implicitWidth: 26
                    implicitHeight: 26
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: qsTr("Open Main Window")
                    onClicked: {
                        if (overlayWindow.controller) {
                            overlayWindow.controller.showMainWindow()
                        }
                    }
                }
                QQC2.ToolButton {
                    icon.name: "window-close"
                    implicitWidth: 26
                    implicitHeight: 26
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
