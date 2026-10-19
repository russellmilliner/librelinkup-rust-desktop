import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import io.github.librelinkup 1.0

Kirigami.ApplicationWindow {
    id: root
    objectName: "librelinkupMainWindow"
    title: qsTr("LibreLinkUp Desktop v%1 — KDE Plasma 6").arg(appController.appVersion)
    width: 980
    height: 680
    minimumWidth: 780
    minimumHeight: 540
    visible: false
    color: Kirigami.Theme.backgroundColor

    readonly property bool isLightTheme: (0.2126 * root.color.r + 0.7152 * root.color.g + 0.0722 * root.color.b) > 0.45
    property string activeTab: "dashboard"

    readonly property var regionOptions: [
        { value: "us", label: "United States (us)" },
        { value: "global", label: "Global (global)" },
        { value: "eu", label: "European Union (eu)" },
        { value: "eu2", label: "European Union 2 (eu2)" },
        { value: "de", label: "Germany (de)" },
        { value: "fr", label: "France (fr)" },
        { value: "ca", label: "Canada (ca)" },
        { value: "au", label: "Australia (au)" },
        { value: "ap", label: "Asia / Pacific (ap)" },
        { value: "jp", label: "Japan (jp)" },
        { value: "ae", label: "United Arab Emirates (ae)" }
    ]

    function regionIndexForCode(code) {
        const clean = String(code || "us").toLowerCase()
        for (let i = 0; i < regionOptions.length; ++i) {
            if (regionOptions[i].value === clean) return i
        }
        return 0
    }

    function openLogViewer() {
        logViewerWindow.openLogWindow()
    }

    onClosing: function(close) {
        close.accepted = false
        appController.hideMainWindow()
    }

    AppController {
        id: appController
        Component.onCompleted: {
            appController.initializeController()
        }
        onGraphPointsJsonChanged: {
            glucoseCanvas.requestPaint()
        }
        onResultUnitChanged: {
            glucoseCanvas.requestPaint()
        }
        onThemeModeChanged: {
            glucoseCanvas.requestPaint()
        }
    }

    Timer {
        id: pollTimer
        interval: Math.max(15, appController.pollIntervalSeconds) * 1000
        repeat: true
        running: appController.loggedIn
        onTriggered: {
            appController.refreshGlucoseNow()
        }
    }

    LogViewerDialog {
        id: logViewerWindow
        controller: appController
    }

    OverlayWidget {
        id: overlayWidgetWindow
        controller: appController
    }

    FileDialog {
        id: customSoundFileDialog
        title: qsTr("Select Custom Alert Audio File")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Audio Files (*.mp3 *.wav *.ogg *.flac)"), qsTr("All Files (*)")]
        onAccepted: {
            customSoundField.text = selectedFile.toString().replace("file://", "")
        }
    }

    header: QQC2.ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.rightMargin: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 2
                text: qsTr("LibreLinkUp Desktop")
                font.bold: true
            }

            Rectangle {
                radius: 6
                color: appController.credentialsEncrypted
                    ? Qt.rgba(0.13, 0.77, 0.37, 0.18)
                    : Qt.rgba(0.58, 0.64, 0.72, 0.18)
                border.color: appController.credentialsEncrypted ? "#22c55e" : "#94a3b8"
                border.width: 1
                implicitWidth: vaultLabel.implicitWidth + 16
                implicitHeight: 24

                QQC2.Label {
                    id: vaultLabel
                    anchors.centerIn: parent
                    text: appController.credentialsEncrypted
                        ? qsTr("🔒 AES-256-GCM Encrypted Vault")
                        : qsTr("🔓 Vault Empty")
                    font.pointSize: 8.5
                    font.bold: true
                    color: Kirigami.Theme.textColor
                }
            }

            Item { Layout.fillWidth: true }

            QQC2.ToolButton {
                icon.name: "view-statistics"
                text: qsTr("Dashboard")
                checkable: true
                checked: root.activeTab === "dashboard"
                onClicked: root.activeTab = "dashboard"
            }

            QQC2.ToolButton {
                icon.name: "configure"
                text: qsTr("General & Tray")
                checkable: true
                checked: root.activeTab === "general"
                onClicked: root.activeTab = "general"
            }

            QQC2.ToolButton {
                icon.name: "preferences-desktop-notification-bell"
                text: qsTr("Alerts")
                checkable: true
                checked: root.activeTab === "alerts"
                onClicked: root.activeTab = "alerts"
            }

            QQC2.ToolButton {
                icon.name: "user-identity"
                text: qsTr("Account")
                checkable: true
                checked: root.activeTab === "account"
                onClicked: root.activeTab = "account"
            }

            QQC2.ToolButton {
                icon.name: "utilities-log-viewer"
                text: qsTr("Log Viewer (%1)").arg(appController.eventLogCount)
                onClicked: root.openLogViewer()
            }

            QQC2.ToolButton {
                icon.name: "view-refresh"
                text: qsTr("Refresh")
                enabled: appController.loggedIn && !appController.busy
                onClicked: appController.refreshGlucoseNow()
            }

            QQC2.ToolButton {
                icon.name: "window-minimize"
                text: qsTr("To Tray")
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Hide window to the KDE system tray")
                onClicked: appController.hideMainWindow()
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        // Error Banner
        Rectangle {
            Layout.fillWidth: true
            visible: appController.errorMessage.length > 0
            implicitHeight: errRow.implicitHeight + 16
            radius: 8
            color: Qt.rgba(0.94, 0.27, 0.27, 0.16)
            border.color: "#ef4444"
            border.width: 1

            RowLayout {
                id: errRow
                anchors.fill: parent
                anchors.margins: 8
                spacing: 10

                Kirigami.Icon {
                    source: "dialog-error"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                }

                QQC2.Label {
                    text: appController.errorMessage
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                }
            }
        }

        // ==================== TAB 1: DASHBOARD / LOGIN ====================
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.activeTab === "dashboard"

            // Login Card when not signed in
            Rectangle {
                anchors.centerIn: parent
                width: Math.min(parent.width - 32, 480)
                implicitHeight: loginCol.implicitHeight + 36
                visible: !appController.loggedIn
                radius: 12
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: Qt.alpha(Kirigami.Theme.textColor, 0.18)
                border.width: 1

                ColumnLayout {
                    id: loginCol
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 14

                    Kirigami.Heading {
                        level: 2
                        text: qsTr("Sign In to LibreLinkUp")
                        color: Kirigami.Theme.textColor
                    }

                    QQC2.Label {
                        text: qsTr("Use your LibreLinkUp follower credentials. All credentials and tokens are encrypted at rest using AES-256-GCM + PBKDF2-HMAC-SHA256.")
                        wrapMode: Text.WordWrap
                        opacity: 0.82
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Email / Username")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.TextField {
                            id: loginEmailField
                            Layout.fillWidth: true
                            placeholderText: qsTr("you@example.com")
                            text: appController.username
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Password")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.TextField {
                            id: loginPasswordField
                            Layout.fillWidth: true
                            echoMode: TextInput.Password
                            placeholderText: qsTr("Enter your LibreLinkUp password")
                            onAccepted: signInButton.clicked()
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Regional Server")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.ComboBox {
                            id: loginRegionCombo
                            Layout.fillWidth: true
                            model: root.regionOptions
                            textRole: "label"
                            valueRole: "value"
                            currentIndex: root.regionIndexForCode(appController.country)
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        QQC2.Button {
                            id: signInButton
                            Layout.fillWidth: true
                            highlighted: true
                            icon.name: "dialog-ok-apply"
                            text: appController.busy ? qsTr("Signing In...") : qsTr("Sign In & Encrypt")
                            enabled: !appController.busy
                            onClicked: {
                                const selectedRegion = root.regionOptions[loginRegionCombo.currentIndex].value
                                appController.login(
                                    loginEmailField.text,
                                    loginPasswordField.text,
                                    selectedRegion,
                                    appController.language
                                )
                            }
                        }

                        QQC2.Button {
                            icon.name: "view-preview"
                            text: qsTr("Preview Sample Data")
                            onClicked: appController.loadDemoSampleData()
                        }
                    }
                }
            }

            // Live Glucose Telemetry View when signed in
            ColumnLayout {
                anchors.fill: parent
                visible: appController.loggedIn
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    // Hero Glucose Reading Card
                    Rectangle {
                        Layout.preferredWidth: 320
                        Layout.fillHeight: true
                        Layout.maximumHeight: 200
                        radius: 14
                        color: appController.rangeColorHex
                        border.color: Qt.rgba(1, 1, 1, 0.35)
                        border.width: 1.5

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 18
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                QQC2.Label {
                                    text: qsTr("CURRENT GLUCOSE")
                                    font.pointSize: 9
                                    font.bold: true
                                    color: Qt.rgba(1, 1, 1, 0.9)
                                }
                                Item { Layout.fillWidth: true }
                                Rectangle {
                                    radius: 6
                                    color: Qt.rgba(0, 0, 0, 0.24)
                                    implicitWidth: statusPillLabel.implicitWidth + 14
                                    implicitHeight: 22
                                    QQC2.Label {
                                        id: statusPillLabel
                                        anchors.centerIn: parent
                                        text: appController.rangeStatusLabel
                                        font.pointSize: 8.5
                                        font.bold: true
                                        color: "#ffffff"
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                QQC2.Label {
                                    text: appController.glucoseDisplay
                                    font.pointSize: 44
                                    font.bold: true
                                    color: "#ffffff"
                                }

                                ColumnLayout {
                                    spacing: 2
                                    QQC2.Label {
                                        text: appController.resultUnit
                                        font.pointSize: 12
                                        font.bold: true
                                        color: "#ffffff"
                                    }
                                    QQC2.Label {
                                        text: appController.trendDescription
                                        font.pointSize: 9.5
                                        color: Qt.rgba(1, 1, 1, 0.92)
                                    }
                                }

                                Item { Layout.fillWidth: true }

                                Rectangle {
                                    width: 54
                                    height: 54
                                    radius: 27
                                    color: Qt.rgba(1, 1, 1, 0.25)
                                    border.color: Qt.rgba(1, 1, 1, 0.5)
                                    border.width: 1.5

                                    QQC2.Label {
                                        anchors.centerIn: parent
                                        text: appController.trendSymbol
                                        font.pointSize: 24
                                        font.bold: true
                                        color: "#ffffff"
                                    }
                                }
                            }

                            Item { Layout.fillHeight: true }

                            QQC2.Label {
                                text: qsTr("Updated: %1").arg(appController.lastUpdated)
                                font.pointSize: 8.5
                                color: Qt.rgba(1, 1, 1, 0.88)
                            }
                        }
                    }

                    // Telemetry Details & Quick Toggles Card
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.maximumHeight: 200
                        radius: 14
                        color: Kirigami.Theme.alternateBackgroundColor
                        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                        border.width: 1

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 18
                            spacing: 10

                            RowLayout {
                                Layout.fillWidth: true
                                Kirigami.Heading {
                                    level: 3
                                    text: appController.patientName.length > 0
                                        ? qsTr("Patient: %1").arg(appController.patientName)
                                        : qsTr("CGM Sensor Overview")
                                    color: Kirigami.Theme.textColor
                                }
                                Item { Layout.fillWidth: true }
                                QQC2.Button {
                                    icon.name: "window-duplicate"
                                    text: appController.overlayVisible
                                        ? qsTr("Hide Desktop Overlay")
                                        : qsTr("Show Desktop Overlay")
                                    onClicked: appController.updateOverlayVisible(!appController.overlayVisible)
                                }
                            }

                            GridLayout {
                                Layout.fillWidth: true
                                columns: 2
                                columnSpacing: 24
                                rowSpacing: 8

                                QQC2.Label {
                                    text: qsTr("Target Range: %1").arg(appController.targetRangeDisplay)
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                }

                                QQC2.Label {
                                    text: qsTr("Region: %1 • Poll: every %2s").arg(appController.country.toUpperCase()).arg(appController.pollIntervalSeconds)
                                    color: Kirigami.Theme.textColor
                                }

                                QQC2.CheckBox {
                                    text: qsTr("Show glucose level in Tray Icon")
                                    checked: appController.showGlucoseInTrayIcon
                                    onToggled: appController.updateShowGlucoseInTrayIcon(checked)
                                }

                                QQC2.CheckBox {
                                    text: qsTr("Show glucose info on Tray Hover")
                                    checked: appController.showGlucoseInTrayTooltip
                                    onToggled: appController.updateShowGlucoseInTrayTooltip(checked)
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                QQC2.Label {
                                    text: qsTr("Theme:")
                                    color: Kirigami.Theme.textColor
                                }
                                QQC2.Button {
                                    text: qsTr("System")
                                    checkable: true
                                    checked: appController.themeMode === "system"
                                    onClicked: appController.updateThemeMode("system")
                                }
                                QQC2.Button {
                                    text: qsTr("Dark")
                                    checkable: true
                                    checked: appController.themeMode === "dark"
                                    onClicked: appController.updateThemeMode("dark")
                                }
                                QQC2.Button {
                                    text: qsTr("Light")
                                    checkable: true
                                    checked: appController.themeMode === "light"
                                    onClicked: appController.updateThemeMode("light")
                                }

                                Item { Layout.fillWidth: true }

                                QQC2.Label {
                                    text: qsTr("Unit:")
                                    color: Kirigami.Theme.textColor
                                }
                                QQC2.Button {
                                    text: "mg/dL"
                                    checkable: true
                                    checked: appController.resultUnit === "mg/dL"
                                    onClicked: appController.updateResultUnit("mg/dL")
                                }
                                QQC2.Button {
                                    text: "mmol/L"
                                    checkable: true
                                    checked: appController.resultUnit === "mmol/L"
                                    onClicked: appController.updateResultUnit("mmol/L")
                                }
                            }
                        }
                    }
                }

                // 12-Hour Glucose Trend Graph Card
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: 14
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            Kirigami.Heading {
                                level: 3
                                text: qsTr("Continuous Glucose Trend (%1)").arg(appController.resultUnit)
                                color: Kirigami.Theme.textColor
                            }
                            Item { Layout.fillWidth: true }
                            QQC2.Label {
                                text: qsTr("Shaded band indicates target range (%1)").arg(appController.targetRangeDisplay)
                                opacity: 0.75
                                color: Kirigami.Theme.textColor
                            }
                        }

                        Canvas {
                            id: glucoseCanvas
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            onWidthChanged: requestPaint()
                            onHeightChanged: requestPaint()

                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.clearRect(0, 0, width, height)

                                let pts = []
                                try {
                                    pts = JSON.parse(appController.graphPointsJson || "[]")
                                } catch (e) {
                                    pts = []
                                }

                                const padLeft = 48
                                const padRight = 20
                                const padTop = 16
                                const padBottom = 28
                                const plotW = Math.max(10, width - padLeft - padRight)
                                const plotH = Math.max(10, height - padTop - padBottom)

                                const isMmol = appController.resultUnit === "mmol/L"
                                const conv = function(mg) {
                                    return isMmol ? (mg / 18.0) : mg
                                }

                                let minMg = 50
                                let maxMg = 260
                                for (let i = 0; i < pts.length; ++i) {
                                    const v = Number(pts[i].value_mg_dl || 0)
                                    if (v > 0) {
                                        if (v - 20 < minMg) minMg = Math.max(30, v - 20)
                                        if (v + 20 > maxMg) maxMg = Math.min(420, v + 20)
                                    }
                                }

                                const yForMg = function(mg) {
                                    const ratio = (mg - minMg) / Math.max(1, (maxMg - minMg))
                                    return padTop + plotH * (1.0 - Math.max(0.0, Math.min(1.0, ratio)))
                                }

                                // Target range shaded band
                                const yHigh = yForMg(appController.targetHighMgDl)
                                const yLow = yForMg(appController.targetLowMgDl)
                                ctx.fillStyle = root.isLightTheme
                                    ? "rgba(34, 197, 94, 0.14)"
                                    : "rgba(34, 197, 94, 0.18)"
                                ctx.fillRect(padLeft, yHigh, plotW, Math.max(2, yLow - yHigh))

                                // Grid lines
                                ctx.strokeStyle = root.isLightTheme
                                    ? "rgba(51, 65, 85, 0.18)"
                                    : "rgba(148, 163, 184, 0.18)"
                                ctx.lineWidth = 1
                                ctx.font = "11px sans-serif"
                                ctx.fillStyle = root.isLightTheme ? "#334155" : "#cbd5e1"

                                const steps = 4
                                for (let s = 0; s <= steps; ++s) {
                                    const mgVal = minMg + ((maxMg - minMg) * s) / steps
                                    const y = yForMg(mgVal)
                                    ctx.beginPath()
                                    ctx.moveTo(padLeft, y)
                                    ctx.lineTo(padLeft + plotW, y)
                                    ctx.stroke()

                                    const labelText = isMmol
                                        ? conv(mgVal).toFixed(1)
                                        : Math.round(mgVal).toString()
                                    ctx.fillText(labelText, 8, y + 4)
                                }

                                if (pts.length === 0) {
                                    ctx.font = "13px sans-serif"
                                    ctx.fillText(
                                        "No CGM graph points received yet — click Refresh or Preview Sample Data.",
                                        padLeft + 24,
                                        padTop + plotH / 2
                                    )
                                    return
                                }

                                // Plot line
                                ctx.strokeStyle = root.isLightTheme ? "#0284c7" : "#38bdf8"
                                ctx.lineWidth = 2.5
                                ctx.beginPath()
                                for (let i = 0; i < pts.length; ++i) {
                                    const x = pts.length === 1
                                        ? (padLeft + plotW / 2)
                                        : (padLeft + (plotW * i) / (pts.length - 1))
                                    const y = yForMg(Number(pts[i].value_mg_dl || 0))
                                    if (i === 0) ctx.moveTo(x, y)
                                    else ctx.lineTo(x, y)
                                }
                                ctx.stroke()

                                // Plot points
                                for (let i = 0; i < pts.length; ++i) {
                                    const v = Number(pts[i].value_mg_dl || 0)
                                    const x = pts.length === 1
                                        ? (padLeft + plotW / 2)
                                        : (padLeft + (plotW * i) / (pts.length - 1))
                                    const y = yForMg(v)

                                    if (v < 70) ctx.fillStyle = "#ef4444"
                                    else if (v > 240) ctx.fillStyle = "#f97316"
                                    else if (v < appController.targetLowMgDl || v > appController.targetHighMgDl) ctx.fillStyle = "#eab308"
                                    else ctx.fillStyle = "#22c55e"

                                    ctx.beginPath()
                                    ctx.arc(x, y, 4, 0, Math.PI * 2)
                                    ctx.fill()
                                }
                            }
                        }
                    }
                }
            }
        }

        // ==================== TAB 2: GENERAL & TRAY SETTINGS ====================
        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.activeTab === "general"

            ColumnLayout {
                width: Math.min(parent.width, 760)
                spacing: 16

                Kirigami.Heading {
                    level: 2
                    text: qsTr("General, Appearance & System Tray Settings")
                    color: Kirigami.Theme.textColor
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: trayGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: trayGroupCol
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 12

                        Kirigami.Heading {
                            level: 3
                            text: qsTr("System Tray & Startup Behavior")
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Switch {
                            text: qsTr("Show blood glucose level directly inside the System Tray icon badge")
                            checked: appController.showGlucoseInTrayIcon
                            onToggled: appController.updateShowGlucoseInTrayIcon(checked)
                        }

                        QQC2.Switch {
                            text: qsTr("Show blood glucose telemetry & target range in the System Tray hover tooltip")
                            checked: appController.showGlucoseInTrayTooltip
                            onToggled: appController.updateShowGlucoseInTrayTooltip(checked)
                        }

                        QQC2.Switch {
                            text: qsTr("Launch automatically when logging into KDE Plasma")
                            checked: appController.autostartEnabled
                            onToggled: appController.updateAutostartEnabled(checked)
                        }

                        QQC2.Switch {
                            text: qsTr("Start minimized to the System Tray on login (no full-screen popup)")
                            checked: appController.startMinimized
                            onToggled: appController.updateStartMinimized(checked)
                        }
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: displayGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: displayGroupCol
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 12

                        Kirigami.Heading {
                            level: 3
                            text: qsTr("Theme, Units & Window Mode")
                            color: Kirigami.Theme.textColor
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            QQC2.Label {
                                text: qsTr("Color Theme:")
                                Layout.preferredWidth: 200
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                model: [
                                    { label: qsTr("System (Follow KDE Plasma)"), value: "system" },
                                    { label: qsTr("Dark Mode (Breeze Dark)"), value: "dark" },
                                    { label: qsTr("Light Mode (Breeze Light)"), value: "light" }
                                ]
                                textRole: "label"
                                valueRole: "value"
                                currentIndex: appController.themeMode === "dark" ? 1 : (appController.themeMode === "light" ? 2 : 0)
                                onActivated: appController.updateThemeMode(currentValue)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            QQC2.Label {
                                text: qsTr("Glucose Unit:")
                                Layout.preferredWidth: 200
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                model: ["mg/dL", "mmol/L"]
                                currentIndex: appController.resultUnit === "mmol/L" ? 1 : 0
                                onActivated: appController.updateResultUnit(currentText)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            QQC2.Label {
                                text: qsTr("Window Mode:")
                                Layout.preferredWidth: 200
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                model: [
                                    { label: qsTr("Standard Windowed"), value: "windowed" },
                                    { label: qsTr("Floating Desktop Overlay"), value: "overlay" },
                                    { label: qsTr("Transparent Desktop Overlay"), value: "overlayTransparent" }
                                ]
                                textRole: "label"
                                valueRole: "value"
                                currentIndex: appController.windowMode === "overlay"
                                    ? 1
                                    : (appController.windowMode === "overlayTransparent" ? 2 : 0)
                                onActivated: appController.updateWindowMode(currentValue)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            QQC2.Label {
                                text: qsTr("Polling Interval (seconds):")
                                Layout.preferredWidth: 200
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.SpinBox {
                                from: 15
                                to: 600
                                stepSize: 15
                                value: appController.pollIntervalSeconds
                                onValueModified: appController.updatePollIntervalSeconds(value)
                            }
                        }
                    }
                }
            }
        }

        // ==================== TAB 3: ALERTS & THRESHOLDS ====================
        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.activeTab === "alerts"

            ColumnLayout {
                width: Math.min(parent.width, 760)
                spacing: 16

                Kirigami.Heading {
                    level: 2
                    text: qsTr("Glucose Alerts & Custom Thresholds")
                    color: Kirigami.Theme.textColor
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: alertsGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: alertsGroupCol
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 12

                        QQC2.Switch {
                            id: bringToFrontSwitch
                            text: qsTr("Bring main window to front when glucose is out of range")
                            checked: appController.bringToFrontEnabled
                        }

                        QQC2.Switch {
                            id: flashWindowSwitch
                            text: qsTr("Flash taskbar / window attention on out-of-range reading")
                            checked: appController.flashWindowEnabled
                        }

                        QQC2.Switch {
                            id: audioAlertSwitch
                            text: qsTr("Play audio alert when glucose is out of range")
                            checked: appController.audioAlertEnabled
                        }

                        QQC2.Switch {
                            id: customSoundSwitch
                            visible: audioAlertSwitch.checked
                            text: qsTr("Use custom audio file (.mp3 / .wav / .ogg)")
                            checked: appController.useCustomSound
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: audioAlertSwitch.checked && customSoundSwitch.checked
                            spacing: 8

                            QQC2.TextField {
                                id: customSoundField
                                Layout.fillWidth: true
                                text: appController.customSoundPath
                                placeholderText: qsTr("/path/to/custom-alert.mp3")
                            }

                            QQC2.Button {
                                icon.name: "document-open"
                                text: qsTr("Browse...")
                                onClicked: customSoundFileDialog.open()
                            }
                        }

                        QQC2.Switch {
                            id: overrideThresholdSwitch
                            text: qsTr("Override sensor target range with custom alert thresholds (mg/dL)")
                            checked: appController.overrideThreshold
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: overrideThresholdSwitch.checked
                            spacing: 18

                            ColumnLayout {
                                QQC2.Label {
                                    text: qsTr("Low Alert Threshold (mg/dL)")
                                    color: Kirigami.Theme.textColor
                                }
                                QQC2.SpinBox {
                                    id: customLowSpin
                                    from: 40
                                    to: 180
                                    value: Math.round(appController.customTargetLow)
                                }
                            }

                            ColumnLayout {
                                QQC2.Label {
                                    text: qsTr("High Alert Threshold (mg/dL)")
                                    color: Kirigami.Theme.textColor
                                }
                                QQC2.SpinBox {
                                    id: customHighSpin
                                    from: 100
                                    to: 400
                                    value: Math.round(appController.customTargetHigh)
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            QQC2.Button {
                                highlighted: true
                                icon.name: "document-save"
                                text: qsTr("Save Alert Settings")
                                onClicked: {
                                    appController.saveAlertSettings(
                                        bringToFrontSwitch.checked,
                                        flashWindowSwitch.checked,
                                        audioAlertSwitch.checked,
                                        customSoundSwitch.checked,
                                        customSoundField.text,
                                        overrideThresholdSwitch.checked,
                                        customLowSpin.value,
                                        customHighSpin.value
                                    )
                                }
                            }

                            QQC2.Button {
                                icon.name: "media-playback-start"
                                text: qsTr("Test Alert Now")
                                onClicked: appController.testAlertNow()
                            }
                        }
                    }
                }
            }
        }

        // ==================== TAB 4: ACCOUNT & SECURITY ====================
        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.activeTab === "account"

            ColumnLayout {
                width: Math.min(parent.width, 760)
                spacing: 16

                Kirigami.Heading {
                    level: 2
                    text: qsTr("Account & Encrypted Credential Vault")
                    color: Kirigami.Theme.textColor
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: accountCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: accountCol
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 10

                        QQC2.Label {
                            text: qsTr("Session Status: %1").arg(appController.statusMessage)
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Label {
                            text: qsTr("Connected Patient: %1").arg(
                                appController.patientName.length > 0 ? appController.patientName : qsTr("None")
                            )
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Label {
                            text: qsTr("Regional Endpoint: %1").arg(appController.country.toUpperCase())
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Label {
                            text: qsTr("Encryption Cipher: AES-256-GCM (PBKDF2-HMAC-SHA256, 100,000 iterations, 0600 permissions)")
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Label {
                            text: qsTr("Encrypted Vault Path: %1").arg(appController.credentialsVaultPath)
                            font.family: "monospace"
                            wrapMode: Text.WrapAnywhere
                            color: Kirigami.Theme.textColor
                        }

                        RowLayout {
                            spacing: 10
                            QQC2.Button {
                                icon.name: "system-log-out"
                                text: qsTr("Sign Out & Wipe Encrypted Vault")
                                enabled: appController.loggedIn || appController.credentialsEncrypted
                                onClicked: {
                                    appController.logout()
                                    root.activeTab = "dashboard"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
