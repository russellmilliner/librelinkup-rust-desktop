import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Window {
    id: overlayWindow
    objectName: "librelinkupOverlayWindow"
    title: qsTr("LibreLinkUp Rust Desktop Overlay")
    width: controller ? Math.max(240, Math.min(600, controller.overlayWidth)) : 276
    height: controller ? Math.max(80, Math.min(260, controller.overlayHeight)) : 88
    minimumWidth: 240
    minimumHeight: 80
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"
    visible: Boolean(controller && controller.overlayVisible)

    property var controller: null
    property bool placementReady: false
    readonly property bool isDarkOverlay: Boolean(controller ? controller.overlayDarkTheme : true)
    readonly property int opacityPercent: controller ? Math.max(20, Math.min(100, controller.overlayOpacityPercent)) : 88
    readonly property real opacityAlpha: opacityPercent / 100.0
    readonly property color statusAccentColor: Qt.color(controller ? controller.rangeColorHex : "#22c55e")

    function clampAndPersistPlacement() {
        if (!overlayWindow.visible || !overlayWindow.placementReady || !overlayWindow.controller) {
            return
        }
        const sc = overlayWindow.screen
        if (!sc) {
            return
        }
        const scName = String(sc.name || "")
        const scX = Number(sc.virtualX !== undefined ? sc.virtualX : 0)
        const scY = Number(sc.virtualY !== undefined ? sc.virtualY : 0)
        const scW = Number(sc.width || 1920)
        const scH = Number(sc.height || 1080)

        // Ensure the overlay window never overflows outside its monitor bounds
        const minX = scX
        const maxX = Math.max(scX, scX + scW - overlayWindow.width)
        const minY = scY
        const maxY = Math.max(scY, scY + scH - overlayWindow.height)

        const clampedGlobalX = Math.max(minX, Math.min(maxX, overlayWindow.x))
        const clampedGlobalY = Math.max(minY, Math.min(maxY, overlayWindow.y))

        if (Math.abs(clampedGlobalX - overlayWindow.x) > 1) {
            overlayWindow.x = clampedGlobalX
        }
        if (Math.abs(clampedGlobalY - overlayWindow.y) > 1) {
            overlayWindow.y = clampedGlobalY
        }

        const relX = Math.round(clampedGlobalX - scX)
        const relY = Math.round(clampedGlobalY - scY)
        overlayWindow.controller.saveOverlayPlacement(
            scName,
            relX,
            relY,
            Math.round(overlayWindow.width),
            Math.round(overlayWindow.height)
        )
    }

    Timer {
        id: savePlacementDebounce
        interval: 220
        repeat: false
        onTriggered: overlayWindow.clampAndPersistPlacement()
    }

    onXChanged: {
        if (overlayWindow.visible && overlayWindow.placementReady) {
            savePlacementDebounce.restart()
        }
    }
    onYChanged: {
        if (overlayWindow.visible && overlayWindow.placementReady) {
            savePlacementDebounce.restart()
        }
    }
    onScreenChanged: {
        if (overlayWindow.visible && overlayWindow.placementReady) {
            savePlacementDebounce.restart()
        }
    }

    Rectangle {
        id: overlayPill
        objectName: "overlayPill"
        anchors.fill: parent
        radius: 14
        clip: true
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
            id: dragMouseArea
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            property point pressPos: Qt.point(0, 0)
            property bool dragging: false

            onPressed: function(mouse) {
                if (mouse.button === Qt.RightButton) {
                    if (overlayWindow.controller) {
                        overlayWindow.controller.popupTrayMenuAtCursor()
                    }
                    return
                }
                pressPos = Qt.point(mouse.x, mouse.y)
                dragging = true
                if (!overlayWindow.startSystemMove()) {
                    // Fallback manual drag handled in onPositionChanged
                }
            }
            onPositionChanged: function(mouse) {
                if (dragging && (mouse.buttons & Qt.LeftButton)) {
                    const dx = mouse.x - pressPos.x
                    const dy = mouse.y - pressPos.y
                    if (Math.abs(dx) > 1 || Math.abs(dy) > 1) {
                        overlayWindow.x += dx
                        overlayWindow.y += dy
                    }
                }
            }
            onReleased: function() {
                if (dragging) {
                    dragging = false
                    overlayWindow.clampAndPersistPlacement()
                }
            }
            onDoubleClicked: function(mouse) {
                if (mouse.button === Qt.LeftButton && overlayWindow.controller) {
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
            id: overlayRow
            objectName: "overlayRow"
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 10
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            spacing: 8

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
                Layout.preferredWidth: 34
                Layout.preferredHeight: 34
                radius: 17
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
                    font.pointSize: 14
                    font.bold: true
                    color: "#ffffff"
                }
            }

            GridLayout {
                columns: 2
                rowSpacing: 4
                columnSpacing: 4

                // 1. Toggle Dark / Color Overlay Theme
                Rectangle {
                    id: overlayThemeToggleBtn
                    objectName: "overlayThemeToggleBtn"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                    implicitWidth: 22
                    implicitHeight: 22
                    radius: 6
                    color: themeBtnMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14)
                    border.color: Qt.rgba(1, 1, 1, 0.3)
                    border.width: 1

                    QQC2.Label {
                        anchors.centerIn: parent
                        text: overlayWindow.isDarkOverlay ? "🌙" : "🎨"
                        font.pointSize: 8.5
                        color: "#ffffff"
                    }

                    MouseArea {
                        id: themeBtnMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (overlayWindow.controller) {
                                overlayWindow.controller.updateOverlayDarkTheme(!overlayWindow.isDarkOverlay)
                            }
                        }
                    }

                    QQC2.ToolTip.visible: themeBtnMouse.containsMouse
                    QQC2.ToolTip.text: overlayWindow.isDarkOverlay
                        ? qsTr("Overlay Dark Theme ON (Click for Color Fill • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                        : qsTr("Overlay Color Fill ON (Click for Dark Theme • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                }

                // 2. Open Main Window
                Rectangle {
                    id: overlayRestoreBtn
                    objectName: "overlayRestoreBtn"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                    implicitWidth: 22
                    implicitHeight: 22
                    radius: 6
                    color: restoreBtnMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14)
                    border.color: Qt.rgba(1, 1, 1, 0.3)
                    border.width: 1

                    QQC2.Label {
                        anchors.centerIn: parent
                        text: "↗"
                        font.pointSize: 9.5
                        font.bold: true
                        color: "#ffffff"
                    }

                    MouseArea {
                        id: restoreBtnMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (overlayWindow.controller) {
                                overlayWindow.controller.showMainWindow()
                            }
                        }
                    }

                    QQC2.ToolTip.visible: restoreBtnMouse.containsMouse
                    QQC2.ToolTip.text: qsTr("Open LibreLinkUp Rust Desktop")
                }

                // 3. Context Menu button (accessible even when tray icon is hidden in overlay-only mode)
                Rectangle {
                    id: overlayMenuBtn
                    objectName: "overlayMenuBtn"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                    implicitWidth: 22
                    implicitHeight: 22
                    radius: 6
                    color: menuBtnMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14)
                    border.color: Qt.rgba(1, 1, 1, 0.3)
                    border.width: 1

                    QQC2.Label {
                        anchors.centerIn: parent
                        text: "☰"
                        font.pointSize: 8.5
                        font.bold: true
                        color: "#ffffff"
                    }

                    MouseArea {
                        id: menuBtnMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (overlayWindow.controller) {
                                overlayWindow.controller.popupTrayMenuAtCursor()
                            }
                        }
                    }

                    QQC2.ToolTip.visible: menuBtnMouse.containsMouse
                    QQC2.ToolTip.text: qsTr("Open Menu (or right-click anywhere on overlay)")
                }

                // 4. Hide Overlay
                Rectangle {
                    id: overlayCloseBtn
                    objectName: "overlayCloseBtn"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                    implicitWidth: 22
                    implicitHeight: 22
                    radius: 6
                    color: closeBtnMouse.containsMouse ? Qt.rgba(0.94, 0.27, 0.27, 0.65) : Qt.rgba(1, 1, 1, 0.14)
                    border.color: Qt.rgba(1, 1, 1, 0.3)
                    border.width: 1

                    QQC2.Label {
                        anchors.centerIn: parent
                        text: "✕"
                        font.pointSize: 8.5
                        font.bold: true
                        color: "#ffffff"
                    }

                    MouseArea {
                        id: closeBtnMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (overlayWindow.controller) {
                                overlayWindow.controller.updateOverlayVisible(false)
                            }
                        }
                    }

                    QQC2.ToolTip.visible: closeBtnMouse.containsMouse
                    QQC2.ToolTip.text: qsTr("Hide Overlay")
                }
            }
        }
    }
}
