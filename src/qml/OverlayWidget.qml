import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Window {
    id: overlayWindow
    objectName: "librelinkupOverlayWindow"
    title: onAllDesktops
        ? qsTr("LibreLinkUp Rust Desktop Overlay")
        : qsTr("LibreLinkUp Rust Desktop Overlay [SingleDesktop]")
    width: settings ? Math.max(240, Math.min(600, settings.overlayWidth)) : 276
    height: settings ? Math.max(80, Math.min(260, settings.overlayHeight)) : 88
    minimumWidth: 240
    minimumHeight: 80
    flags: Qt.Tool
        | Qt.FramelessWindowHint
        | Qt.WindowStaysOnTopHint
        | (onAllDesktops ? Qt.BypassWindowManagerHint : Qt.Widget)
    color: "transparent"
    visible: Boolean(settings && settings.overlayVisible)

    /// Settings projection (overlay geometry, theme, toggles).
    property var settings: null
    /// Telemetry projection (glucose value, trend, range colour).
    property var telemetry: null
    property bool placementReady: false
    readonly property bool isDarkOverlay: Boolean(settings ? settings.overlayDarkTheme : true)
    readonly property bool onAllDesktops: Boolean(settings ? settings.overlayOnAllDesktops : true)
    onOnAllDesktopsChanged: allDesktopsCanvas.requestPaint()
    readonly property int opacityPercent: settings ? Math.max(20, Math.min(100, settings.overlayOpacityPercent)) : 88
    readonly property real opacityAlpha: opacityPercent / 100.0
    readonly property color statusAccentColor: Qt.color(telemetry ? telemetry.rangeColorHex : "#22c55e")
    readonly property color lightOverlayBgColor: Qt.darker(statusAccentColor, 1.42)
    readonly property color lightOverlayBorderColor: Qt.darker(statusAccentColor, 1.95)

    Rectangle {
        id: overlayPill
        objectName: "overlayPill"
        anchors.fill: parent
        radius: 14
        clip: true
        Accessible.role: Accessible.Pane
        Accessible.name: telemetry ? telemetry.accessibleSummary : qsTr("LibreLinkUp Glucose Overlay")
        Accessible.description: qsTr("Floating desktop glucose overlay. Double-click to open main window, right-click for menu, scroll wheel to adjust opacity.")
        color: overlayWindow.isDarkOverlay
            ? Qt.rgba(0.06, 0.09, 0.16, overlayWindow.opacityAlpha)
            : Qt.rgba(
                overlayWindow.lightOverlayBgColor.r,
                overlayWindow.lightOverlayBgColor.g,
                overlayWindow.lightOverlayBgColor.b,
                overlayWindow.opacityAlpha
            )
        border.color: overlayWindow.isDarkOverlay
            ? Qt.rgba(
                overlayWindow.statusAccentColor.r,
                overlayWindow.statusAccentColor.g,
                overlayWindow.statusAccentColor.b,
                Math.min(1.0, overlayWindow.opacityAlpha + 0.15)
            )
            : overlayWindow.lightOverlayBorderColor
        border.width: 2.0

        MouseArea {
            id: dragMouseArea
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            property bool dragging: false

            onPressed: function(mouse) {
                if (mouse.button === Qt.RightButton) {
                    if (overlayWindow.settings) {
                        overlayWindow.settings.popupTrayMenuAtCursor()
                    }
                    return
                }
                if (mouse.button === Qt.LeftButton && overlayWindow.settings) {
                    dragging = true
                    overlayWindow.settings.beginOverlayDrag()
                }
            }
            onPositionChanged: function(mouse) {
                if (dragging && (mouse.buttons & Qt.LeftButton) && overlayWindow.settings) {
                    overlayWindow.settings.updateOverlayDrag()
                }
            }
            onReleased: function() {
                if (dragging) {
                    dragging = false
                    if (overlayWindow.settings) {
                        overlayWindow.settings.endOverlayDrag()
                    }
                }
            }
            onCanceled: function() {
                if (dragging) {
                    dragging = false
                    if (overlayWindow.settings) {
                        overlayWindow.settings.endOverlayDrag()
                    }
                }
            }
            onDoubleClicked: function(mouse) {
                if (mouse.button === Qt.LeftButton && overlayWindow.settings) {
                    overlayWindow.settings.showMainWindow()
                }
            }
            onWheel: function(wheel) {
                if (overlayWindow.settings) {
                    const step = wheel.angleDelta.y > 0 ? 5 : -5
                    const nextVal = Math.max(20, Math.min(100, overlayWindow.opacityPercent + step))
                    overlayWindow.settings.updateOverlayOpacityPercent(nextVal)
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
                        text: telemetry ? telemetry.glucoseDisplay : "--"
                        font.pointSize: 20
                        font.bold: true
                        color: "#ffffff"
                    }

                    QQC2.Label {
                        text: telemetry ? telemetry.resultUnit : "mg/dL"
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
                    text: telemetry
                        ? (telemetry.rangeStatusLabel + " • " + telemetry.trendDescription)
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
                    : Qt.rgba(0, 0, 0, 0.34)
                border.color: overlayWindow.isDarkOverlay
                    ? overlayWindow.statusAccentColor
                    : Qt.rgba(1, 1, 1, 0.85)
                border.width: 1.5

                QQC2.Label {
                    anchors.centerIn: parent
                    text: telemetry ? telemetry.trendSymbol : "→"
                    font.pointSize: 14
                    font.bold: true
                    color: "#ffffff"
                }
            }

            ColumnLayout {
                spacing: 4

                RowLayout {
                    spacing: 4

                    // 1. Toggle Dark / Color Overlay Theme
                    Rectangle {
                        id: overlayThemeToggleBtn
                        objectName: "overlayThemeToggleBtn"
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 22
                        implicitWidth: 20
                        implicitHeight: 22
                        radius: 6
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: overlayWindow.isDarkOverlay
                            ? qsTr("Switch overlay to color fill theme")
                            : qsTr("Switch overlay to dark slate theme")
                        Keys.onReturnPressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayDarkTheme(!overlayWindow.isDarkOverlay)
                        Keys.onSpacePressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayDarkTheme(!overlayWindow.isDarkOverlay)
                        color: overlayWindow.isDarkOverlay
                            ? (themeBtnMouse.containsMouse || activeFocus ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14))
                            : (themeBtnMouse.containsMouse || activeFocus ? Qt.rgba(0, 0, 0, 0.52) : Qt.rgba(0, 0, 0, 0.34))
                        border.color: activeFocus
                            ? "#ffffff"
                            : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(1, 1, 1, 0.72))
                        border.width: activeFocus ? 2 : 1

                        Canvas {
                            id: themeToggleCanvas
                            width: 14
                            height: 14
                            anchors.centerIn: parent
                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.reset()
                                if (overlayWindow.isDarkOverlay) {
                                    // Crescent moon icon
                                    ctx.fillStyle = "#fde047"
                                    ctx.beginPath()
                                    ctx.arc(7, 7, 4.8, 0.35, Math.PI * 1.65, false)
                                    ctx.arc(9.2, 5.6, 4.2, Math.PI * 1.48, 0.52, true)
                                    ctx.closePath()
                                    ctx.fill()
                                } else {
                                    // Sun / color theme icon
                                    ctx.fillStyle = "#fef08a"
                                    ctx.strokeStyle = "#ffffff"
                                    ctx.lineWidth = 1.2
                                    ctx.beginPath()
                                    ctx.arc(7, 7, 3.2, 0, Math.PI * 2)
                                    ctx.fill()
                                    for (let a = 0; a < 8; ++a) {
                                        const ang = (a * Math.PI) / 4
                                        ctx.beginPath()
                                        ctx.moveTo(7 + Math.cos(ang) * 4.4, 7 + Math.sin(ang) * 4.4)
                                        ctx.lineTo(7 + Math.cos(ang) * 6.0, 7 + Math.sin(ang) * 6.0)
                                        ctx.stroke()
                                    }
                                }
                            }
                        }

                        Connections {
                            target: overlayWindow
                            function onIsDarkOverlayChanged() {
                                themeToggleCanvas.requestPaint()
                            }
                        }

                        MouseArea {
                            id: themeBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (overlayWindow.settings) {
                                    overlayWindow.settings.updateOverlayDarkTheme(!overlayWindow.isDarkOverlay)
                                }
                            }
                        }

                        QQC2.ToolTip.visible: themeBtnMouse.containsMouse
                        QQC2.ToolTip.text: overlayWindow.isDarkOverlay
                            ? qsTr("Overlay Dark Theme ON (Click for Color Fill • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                            : qsTr("Overlay Color Fill ON (Click for Dark Theme • Scroll wheel adjusts %1% opacity)").arg(overlayWindow.opacityPercent)
                    }

                    // 2. Toggle Show on All Virtual Desktops
                    Rectangle {
                        id: overlayAllDesktopsBtn
                        objectName: "overlayAllDesktopsBtn"
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 22
                        implicitWidth: 20
                        implicitHeight: 22
                        radius: 6
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: overlayWindow.onAllDesktops
                            ? qsTr("Show overlay on all virtual desktops: enabled")
                            : qsTr("Show overlay on all virtual desktops: disabled")
                        Keys.onReturnPressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayOnAllDesktops(!overlayWindow.onAllDesktops)
                        Keys.onSpacePressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayOnAllDesktops(!overlayWindow.onAllDesktops)
                        color: overlayWindow.onAllDesktops
                            ? (overlayWindow.isDarkOverlay
                                ? (allDeskBtnMouse.containsMouse || activeFocus ? Qt.rgba(0.22, 0.74, 0.97, 0.38) : Qt.rgba(0.22, 0.74, 0.97, 0.24))
                                : (allDeskBtnMouse.containsMouse || activeFocus ? Qt.rgba(0, 0, 0, 0.56) : Qt.rgba(0, 0, 0, 0.40)))
                            : (overlayWindow.isDarkOverlay
                                ? (allDeskBtnMouse.containsMouse || activeFocus ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14))
                                : (allDeskBtnMouse.containsMouse || activeFocus ? Qt.rgba(0, 0, 0, 0.52) : Qt.rgba(0, 0, 0, 0.34)))
                        border.color: activeFocus
                            ? "#ffffff"
                            : (overlayWindow.onAllDesktops
                                ? (overlayWindow.isDarkOverlay ? "#38bdf8" : "#ffffff")
                                : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(1, 1, 1, 0.72)))
                        border.width: activeFocus ? 2 : 1

                        Canvas {
                            id: allDesktopsCanvas
                            width: 14
                            height: 14
                            anchors.centerIn: parent
                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.reset()
                                const allDesk = overlayWindow.onAllDesktops
                                const activeCol = allDesk ? "#38bdf8" : "#ffffff"
                                const dimCol = "rgba(255, 255, 255, 0.40)"
                                ctx.lineWidth = 1.25
                                ctx.lineJoin = "round"

                                const cells = [
                                    { x: 1.5, y: 1.5, active: true },
                                    { x: 8.0, y: 1.5, active: allDesk },
                                    { x: 1.5, y: 8.0, active: allDesk },
                                    { x: 8.0, y: 8.0, active: allDesk }
                                ]
                                for (let i = 0; i < cells.length; ++i) {
                                    const c = cells[i]
                                    ctx.strokeStyle = c.active ? activeCol : dimCol
                                    ctx.fillStyle = c.active
                                        ? (allDesk ? "rgba(56, 189, 248, 0.45)" : "#ffffff")
                                        : "transparent"
                                    ctx.beginPath()
                                    ctx.rect(c.x, c.y, 4.5, 4.5)
                                    if (c.active) {
                                        ctx.fill()
                                    }
                                    ctx.stroke()
                                }
                            }
                        }

                        MouseArea {
                            id: allDeskBtnMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (overlayWindow.settings) {
                                    overlayWindow.settings.updateOverlayOnAllDesktops(!overlayWindow.onAllDesktops)
                                }
                            }
                        }

                        QQC2.ToolTip.visible: allDeskBtnMouse.containsMouse
                        QQC2.ToolTip.text: overlayWindow.onAllDesktops
                            ? qsTr("On All Desktops: ON (Visible across all virtual desktops • Click for current desktop only)")
                            : qsTr("On All Desktops: OFF (Current virtual desktop only • Click to show on all desktops)")
                    }

                    // 3. Open Main Window
                    Rectangle {
                        id: overlayRestoreBtn
                        objectName: "overlayRestoreBtn"
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 22
                        implicitWidth: 20
                        implicitHeight: 22
                        radius: 6
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: qsTr("Open LibreLinkUp Rust Desktop main window")
                        Keys.onReturnPressed: if (overlayWindow.settings) overlayWindow.settings.showMainWindow()
                        Keys.onSpacePressed: if (overlayWindow.settings) overlayWindow.settings.showMainWindow()
                        color: overlayWindow.isDarkOverlay
                            ? (restoreBtnMouse.containsMouse || activeFocus ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14))
                            : (restoreBtnMouse.containsMouse || activeFocus ? Qt.rgba(0, 0, 0, 0.52) : Qt.rgba(0, 0, 0, 0.34))
                        border.color: activeFocus
                            ? "#ffffff"
                            : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(1, 1, 1, 0.72))
                        border.width: activeFocus ? 2 : 1

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
                                if (overlayWindow.settings) {
                                    overlayWindow.settings.showMainWindow()
                                }
                            }
                        }

                        QQC2.ToolTip.visible: restoreBtnMouse.containsMouse
                        QQC2.ToolTip.text: qsTr("Open LibreLinkUp Rust Desktop")
                    }
                }

                RowLayout {
                    spacing: 4

                    // 4. Context Menu button (accessible even when tray icon is hidden in overlay-only mode)
                    Rectangle {
                        id: overlayMenuBtn
                        objectName: "overlayMenuBtn"
                        Layout.preferredWidth: 32
                        Layout.preferredHeight: 22
                        implicitWidth: 32
                        implicitHeight: 22
                        radius: 6
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: qsTr("Open LibreLinkUp context menu")
                        Keys.onReturnPressed: if (overlayWindow.settings) overlayWindow.settings.popupTrayMenuAtCursor()
                        Keys.onSpacePressed: if (overlayWindow.settings) overlayWindow.settings.popupTrayMenuAtCursor()
                        color: overlayWindow.isDarkOverlay
                            ? (menuBtnMouse.containsMouse || activeFocus ? Qt.rgba(1, 1, 1, 0.28) : Qt.rgba(1, 1, 1, 0.14))
                            : (menuBtnMouse.containsMouse || activeFocus ? Qt.rgba(0, 0, 0, 0.52) : Qt.rgba(0, 0, 0, 0.34))
                        border.color: activeFocus
                            ? "#ffffff"
                            : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(1, 1, 1, 0.72))
                        border.width: activeFocus ? 2 : 1

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
                                if (overlayWindow.settings) {
                                    overlayWindow.settings.popupTrayMenuAtCursor()
                                }
                            }
                        }

                        QQC2.ToolTip.visible: menuBtnMouse.containsMouse
                        QQC2.ToolTip.text: qsTr("Open Menu (or right-click anywhere on overlay)")
                    }

                    // 5. Hide Overlay
                    Rectangle {
                        id: overlayCloseBtn
                        objectName: "overlayCloseBtn"
                        Layout.preferredWidth: 32
                        Layout.preferredHeight: 22
                        implicitWidth: 32
                        implicitHeight: 22
                        radius: 6
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: qsTr("Hide floating desktop overlay")
                        Keys.onReturnPressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayVisible(false)
                        Keys.onSpacePressed: if (overlayWindow.settings) overlayWindow.settings.updateOverlayVisible(false)
                        color: closeBtnMouse.containsMouse || activeFocus
                            ? Qt.rgba(0.94, 0.27, 0.27, 0.75)
                            : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.14) : Qt.rgba(0, 0, 0, 0.34))
                        border.color: activeFocus
                            ? "#ffffff"
                            : (overlayWindow.isDarkOverlay ? Qt.rgba(1, 1, 1, 0.3) : Qt.rgba(1, 1, 1, 0.72))
                        border.width: activeFocus ? 2 : 1

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
                                if (overlayWindow.settings) {
                                    overlayWindow.settings.updateOverlayVisible(false)
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
}
