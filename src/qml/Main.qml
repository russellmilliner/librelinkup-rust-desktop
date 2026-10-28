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
    title: qsTr("LibreLinkUp Rust Desktop v%1 — KDE Plasma 6").arg(appController.appVersion)
    width: 980
    height: 680
    minimumWidth: 780
    minimumHeight: 540
    visible: false
    color: Kirigami.Theme.backgroundColor

    readonly property bool isLightTheme: (0.2126 * root.color.r + 0.7152 * root.color.g + 0.0722 * root.color.b) > 0.45

    // Width the header needs with every label shown, measured from the real layout at
    // startup (depends on fonts/scaling, so it can't be hard-coded). 0 = not measured yet.
    property real fullHeaderWidth: 0
    readonly property real headerSideMargins: 16
    // Collapse header labels to icons only when the window is narrower than the full header.
    readonly property bool headerCompact: fullHeaderWidth > 0
        && root.width < fullHeaderWidth + headerSideMargins

    function measureHeaderAndFitWindow() {
        fullHeaderWidth = 0 // force full labels for the measurement
        Qt.callLater(function() {
            fullHeaderWidth = Math.ceil(headerRow.implicitWidth)
            const needed = fullHeaderWidth + headerSideMargins
            const maxWidth = Screen.desktopAvailableWidth > 0 ? Screen.desktopAvailableWidth - 40 : needed
            if (root.width < needed) {
                root.width = Math.min(needed, maxWidth)
            }
        })
    }

    Component.onCompleted: measureHeaderAndFitWindow()
    property string activeTab: "dashboard"
    readonly property alias controller: appController

    readonly property var regionOptions: [
        { value: "us", label: "United States (us)" },
        { value: "global", label: "Global (global)" },
        { value: "eu", label: "European Union (eu)" },
        { value: "eu2", label: "United Kingdom & Ireland (eu2)" },
        { value: "de", label: "Germany (de)" },
        { value: "fr", label: "France (fr)" },
        { value: "ca", label: "Canada (ca)" },
        { value: "au", label: "Australia & New Zealand (au)" },
        { value: "ap", label: "Asia / Pacific (ap)" },
        { value: "jp", label: "Japan (jp)" },
        { value: "ae", label: "United Arab Emirates (ae)" },
        { value: "la", label: "Latin America (la)" }
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

    function openAboutDialog() {
        aboutDialogWindow.openAboutWindow()
    }

    onClosing: function(close) {
        close.accepted = false
        appController.hideMainWindow()
    }

    AppController {
        id: appController
        objectName: "librelinkupAppController"
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

    // Re-evaluates reading age every 30 s so stale data / signal loss is flagged (and
    // alerted) even when fetches keep failing or the sensor stops uploading.
    Timer {
        id: stalenessTimer
        interval: 30000
        repeat: true
        running: appController.loggedIn
        onTriggered: appController.evaluateStaleness()
    }

    Timer {
        id: pollTimer
        interval: Math.max(60, appController.pollIntervalSeconds) * 1000
        repeat: true
        running: appController.loggedIn
        onTriggered: {
            appController.refreshGlucoseNow()
        }
    }

    // Periodically check GitHub Releases every 6 hours while running in the background
    Timer {
        id: updateCheckTimer
        interval: 6 * 3600 * 1000
        repeat: true
        running: appController.checkUpdatesOnStartup
        onTriggered: {
            appController.checkForUpdatesSilent()
        }
    }

    LogViewerDialog {
        id: logViewerWindow
        controller: appController
    }

    AboutDialog {
        id: aboutDialogWindow
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
        objectName: "mainHeaderToolBar"
        implicitHeight: headerRow.implicitHeight + 12

        RowLayout {
            id: headerRow
            objectName: "mainHeaderRow"
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            spacing: 2

            Kirigami.Heading {
                level: 4
                text: root.headerCompact ? qsTr("LibreLinkUp") : qsTr("LibreLinkUp Rust")
                font.bold: true
            }

            Rectangle {
                objectName: "vaultStatusBadge"
                radius: 6
                color: appController.credentialsEncrypted
                    ? Qt.rgba(0.13, 0.77, 0.37, 0.18)
                    : Qt.rgba(0.58, 0.64, 0.72, 0.18)
                border.color: appController.credentialsEncrypted ? "#22c55e" : "#94a3b8"
                border.width: 1
                implicitWidth: vaultLabel.implicitWidth + 10
                implicitHeight: 22

                QQC2.Label {
                    id: vaultLabel
                    anchors.centerIn: parent
                    text: root.headerCompact
                        ? (appController.credentialsEncrypted ? "🔒" : "🔓")
                        : (appController.credentialsEncrypted ? qsTr("🔒 AES-256") : qsTr("🔓 Unset"))
                    font.pointSize: 8
                    font.bold: true
                    color: Kirigami.Theme.textColor
                }
            }

            Item { Layout.fillWidth: true }

            QQC2.ToolButton {
                objectName: "tabButtonDashboard"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: text
                icon.name: "view-statistics"
                text: qsTr("Dashboard")
                checkable: true
                checked: root.activeTab === "dashboard"
                onClicked: root.activeTab = "dashboard"
            }

            QQC2.ToolButton {
                objectName: "tabButtonGeneral"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: text
                icon.name: "configure"
                text: qsTr("Settings")
                checkable: true
                checked: root.activeTab === "general"
                onClicked: root.activeTab = "general"
            }

            QQC2.ToolButton {
                objectName: "tabButtonAlerts"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: text
                icon.name: "preferences-desktop-notification-bell"
                text: qsTr("Alerts")
                checkable: true
                checked: root.activeTab === "alerts"
                onClicked: root.activeTab = "alerts"
            }

            QQC2.ToolButton {
                objectName: "tabButtonAccount"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: text
                icon.name: "user-identity"
                text: qsTr("Account")
                checkable: true
                checked: root.activeTab === "account"
                onClicked: root.activeTab = "account"
            }

            QQC2.ToolButton {
                objectName: "tabButtonLogs"
                icon.name: "utilities-log-viewer"
                text: qsTr("Logs (%1)").arg(appController.eventLogCount)
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Open Diagnostic Log Viewer (%1 entries)").arg(appController.eventLogCount)
                onClicked: root.openLogViewer()
            }

            QQC2.ToolButton {
                objectName: "tabButtonAbout"
                icon.name: "help-about"
                text: qsTr("About")
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("About LibreLinkUp Rust Desktop v%1").arg(appController.appVersion)
                onClicked: root.openAboutDialog()
            }

            Rectangle {
                id: headerKofiBadge
                objectName: "headerKofiBadge"
                radius: 12
                color: headerKofiMouse.containsMouse ? "#c81e1e" : "#d93834"
                border.color: "#ffffff33"
                border.width: 1
                implicitWidth: headerKofiLabel.implicitWidth + 12
                implicitHeight: 24

                QQC2.Label {
                    id: headerKofiLabel
                    anchors.centerIn: parent
                    text: root.headerCompact ? qsTr("☕") : qsTr("☕ Ko-fi")
                    font.bold: true
                    font.pointSize: 8.5
                    color: "#ffffff"
                }

                MouseArea {
                    id: headerKofiMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")
                }

                QQC2.ToolTip.visible: headerKofiMouse.containsMouse
                QQC2.ToolTip.text: qsTr("Buy Russell Milliner a coffee on Ko-fi (https://ko-fi.com/russellmilliner)")
            }

            QQC2.ToolButton {
                objectName: "buttonRefreshNow"
                icon.name: "view-refresh"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Refresh glucose telemetry now")
                enabled: appController.loggedIn && !appController.busy
                onClicked: appController.refreshGlucoseNow()
            }

            QQC2.ToolButton {
                objectName: "buttonToTray"
                icon.name: "window-minimize"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Hide window to the KDE system tray")
                onClicked: appController.hideMainWindow()
            }
        }
    }

    ColumnLayout {
        id: mainBodyColumn
        objectName: "mainBodyColumn"
        anchors.fill: parent
        anchors.margins: 14
        spacing: 12

        // Error Banner
        Rectangle {
            id: errorBanner
            objectName: "errorBanner"
            Layout.fillWidth: true
            visible: appController.errorMessage.length > 0
            implicitHeight: errRow.implicitHeight + 16
            radius: 8
            color: Qt.rgba(0.94, 0.27, 0.27, 0.16)
            border.color: "#ef4444"
            border.width: 1

            RowLayout {
                id: errRow
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
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
                    Layout.preferredWidth: 0
                }
            }
        }

        Rectangle {
            id: updateBanner
            objectName: "updateBanner"
            Layout.fillWidth: true
            visible: appController.updateAvailable
            implicitHeight: updateRow.implicitHeight + 16
            radius: 8
            color: Qt.rgba(0.02, 0.52, 0.78, 0.16)
            border.color: "#0284c7"
            border.width: 1

            RowLayout {
                id: updateRow
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 8
                spacing: 10

                Kirigami.Icon {
                    source: "system-software-update"
                    Layout.preferredWidth: 22
                    Layout.preferredHeight: 22
                }

                QQC2.Label {
                    text: qsTr("New version available: %1 (current: v%2)").arg(appController.latestVersion).arg(appController.appVersion)
                    font.bold: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                QQC2.Button {
                    objectName: "bannerInstallUpdateButton"
                    icon.name: "download"
                    text: appController.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(appController.latestVersion)
                    enabled: !appController.installingUpdate
                    onClicked: appController.installUpdate()
                }

                QQC2.Button {
                    objectName: "bannerViewUpdateButton"
                    icon.name: "globe"
                    text: qsTr("View Release")
                    onClicked: Qt.openUrlExternally(appController.updateUrl)
                }
            }
        }

        // ==================== TAB 1: DASHBOARD / LOGIN ====================
        Item {
            id: dashboardContainer
            objectName: "dashboardContainer"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            visible: root.activeTab === "dashboard"

            // Login Card when not signed in
            QQC2.ScrollView {
                id: loginScrollView
                anchors.fill: parent
                visible: !appController.loggedIn
                clip: true
                contentWidth: availableWidth

                Item {
                    width: loginScrollView.availableWidth
                    height: Math.max(loginScrollView.height, loginCard.implicitHeight + 24)
                    implicitHeight: loginCard.implicitHeight + 24

                    Rectangle {
                        id: loginCard
                        objectName: "loginCard"
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 24, 480)
                        implicitHeight: loginCol.implicitHeight + 36
                        radius: 12
                        color: Kirigami.Theme.alternateBackgroundColor
                        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.18)
                        border.width: 1

                        ColumnLayout {
                            id: loginCol
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 18
                            spacing: 12

                            Kirigami.Heading {
                                level: 2
                                text: qsTr("Sign In to LibreLinkUp")
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                wrapMode: Text.WordWrap
                            }

                            QQC2.Label {
                                text: qsTr("Use your LibreLinkUp follower credentials. All credentials and tokens are encrypted at rest using AES-256-GCM + PBKDF2-HMAC-SHA256.")
                                wrapMode: Text.WordWrap
                                opacity: 0.82
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 4
                                QQC2.Label {
                                    text: qsTr("Email / Username")
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
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
                                    Layout.fillWidth: true
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
                                    Layout.fillWidth: true
                                }
                                QQC2.ComboBox {
                                    id: loginRegionCombo
                                    Layout.fillWidth: true
                                    model: root.regionOptions
                                    textRole: "label"
                                    valueRole: "value"
                                    Component.onCompleted: {
                                        currentIndex = root.regionIndexForCode(appController.country)
                                    }
                                    onActivated: function(index) {
                                        if (index >= 0 && index < root.regionOptions.length) {
                                            appController.updateCountry(root.regionOptions[index].value)
                                        }
                                    }
                                    Connections {
                                        target: appController
                                        function onCountryChanged() {
                                            const targetIdx = root.regionIndexForCode(appController.country)
                                            if (loginRegionCombo.currentIndex !== targetIdx) {
                                                loginRegionCombo.currentIndex = targetIdx
                                            }
                                        }
                                    }
                                }
                            }

                            QQC2.CheckBox {
                                objectName: "loginRememberPasswordCheck"
                                text: qsTr("Remember password in encrypted vault (auto-renew expired tokens)")
                                checked: appController.rememberPassword
                                onToggled: appController.updateRememberPassword(checked)
                                Layout.fillWidth: true
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                QQC2.Button {
                                    id: signInButton
                                    Layout.fillWidth: true
                                    highlighted: true
                                    icon.name: "dialog-ok-apply"
                                    text: appController.busy ? qsTr("Signing In...") : qsTr("Sign In & Encrypt")
                                    enabled: !appController.busy
                                    onClicked: {
                                        const idx = loginRegionCombo.currentIndex >= 0 ? loginRegionCombo.currentIndex : 0
                                        const selectedRegion = root.regionOptions[idx].value
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
                                    text: qsTr("Sample Data")
                                    onClicked: appController.loadDemoSampleData()
                                }
                            }
                        }
                    }
                }
            }

            // Live Glucose Telemetry View when signed in
            ColumnLayout {
                id: telemetryMainColumn
                objectName: "telemetryMainColumn"
                anchors.fill: parent
                visible: appController.loggedIn
                spacing: 12

                RowLayout {
                    id: topCardsRow
                    objectName: "topCardsRow"
                    Layout.fillWidth: true
                    spacing: 12

                    readonly property real unifiedCardHeight: Math.max(
                        heroCol.implicitHeight + 28,
                        detailsCol.implicitHeight + 28
                    )

                    // Hero Glucose Reading Card
                    Rectangle {
                        id: heroCard
                        objectName: "heroCard"
                        Layout.preferredWidth: 270
                        Layout.minimumWidth: 250
                        Layout.maximumWidth: 300
                        Layout.preferredHeight: topCardsRow.unifiedCardHeight
                        implicitHeight: topCardsRow.unifiedCardHeight
                        radius: 14
                        color: appController.rangeColorHex
                        border.color: Qt.rgba(1, 1, 1, 0.35)
                        border.width: 1.5

                        ColumnLayout {
                            id: heroCol
                            objectName: "heroCol"
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 14
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                QQC2.Label {
                                    text: qsTr("CURRENT GLUCOSE")
                                    font.pointSize: 8.5
                                    font.bold: true
                                    color: Qt.rgba(1, 1, 1, 0.92)
                                    Layout.fillWidth: true
                                    elide: Text.ElideRight
                                }
                                Rectangle {
                                    radius: 6
                                    color: Qt.rgba(0, 0, 0, 0.24)
                                    implicitWidth: statusPillLabel.implicitWidth + 12
                                    implicitHeight: 20
                                    QQC2.Label {
                                        id: statusPillLabel
                                        anchors.centerIn: parent
                                        text: appController.rangeStatusLabel
                                        font.pointSize: 8
                                        font.bold: true
                                        color: "#ffffff"
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                QQC2.Label {
                                    text: appController.glucoseDisplay
                                    font.pointSize: 34
                                    font.bold: true
                                    color: "#ffffff"
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2
                                    QQC2.Label {
                                        text: appController.resultUnit
                                        font.pointSize: 11
                                        font.bold: true
                                        color: "#ffffff"
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                    QQC2.Label {
                                        text: appController.trendDescription
                                        font.pointSize: 9
                                        color: Qt.rgba(1, 1, 1, 0.92)
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                }

                                Rectangle {
                                    Layout.preferredWidth: 44
                                    Layout.preferredHeight: 44
                                    radius: 22
                                    color: Qt.rgba(1, 1, 1, 0.25)
                                    border.color: Qt.rgba(1, 1, 1, 0.5)
                                    border.width: 1.5

                                    QQC2.Label {
                                        anchors.centerIn: parent
                                        text: appController.trendSymbol
                                        font.pointSize: 20
                                        font.bold: true
                                        color: "#ffffff"
                                    }
                                }
                            }

                            QQC2.Label {
                                text: appController.readingStale
                                      ? qsTr("⚠ STALE — updated: %1").arg(appController.lastUpdated)
                                      : qsTr("Updated: %1").arg(appController.lastUpdated)
                                font.pointSize: 8.5
                                font.bold: appController.readingStale
                                color: Qt.rgba(1, 1, 1, 0.9)
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }

                            QQC2.Label {
                                visible: appController.alertsSnoozed
                                text: "🔕 " + appController.snoozeUntilDisplay
                                font.pointSize: 8.5
                                color: Qt.rgba(1, 1, 1, 0.9)
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                        }
                    }

                    // Telemetry Details & Quick Toggles Card
                    Rectangle {
                        id: detailsCard
                        objectName: "detailsCard"
                        Layout.fillWidth: true
                        Layout.preferredHeight: topCardsRow.unifiedCardHeight
                        implicitHeight: topCardsRow.unifiedCardHeight
                        radius: 14
                        color: Kirigami.Theme.alternateBackgroundColor
                        border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                        border.width: 1

                        ColumnLayout {
                            id: detailsCol
                            objectName: "detailsCol"
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 14
                            spacing: 6

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Kirigami.Heading {
                                    level: 3
                                    text: appController.patientName.length > 0
                                        ? qsTr("Patient: %1").arg(appController.patientName)
                                        : qsTr("CGM Sensor Overview")
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    elide: Text.ElideRight
                                }
                                QQC2.ToolButton {
                                    icon.name: appController.overlayDarkTheme ? "weather-clear-night" : "color-picker"
                                    text: qsTr("Overlay Dark")
                                    checkable: true
                                    checked: appController.overlayDarkTheme
                                    QQC2.ToolTip.visible: hovered
                                    QQC2.ToolTip.text: qsTr("Toggle dark slate background for the floating desktop overlay")
                                    onClicked: appController.updateOverlayDarkTheme(!appController.overlayDarkTheme)
                                }
                                QQC2.ToolButton {
                                    icon.name: "window-duplicate"
                                    text: appController.overlayVisible
                                        ? qsTr("Hide Overlay")
                                        : qsTr("Overlay")
                                    onClicked: appController.updateOverlayVisible(!appController.overlayVisible)
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                QQC2.Label {
                                    text: qsTr("Target: %1").arg(appController.targetRangeDisplay)
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    elide: Text.ElideRight
                                }

                                QQC2.Label {
                                    text: qsTr("Region: %1 • Poll: %2s").arg(appController.country.toUpperCase()).arg(appController.pollIntervalSeconds)
                                    color: Kirigami.Theme.textColor
                                    elide: Text.ElideRight
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                QQC2.CheckBox {
                                    text: qsTr("Tray Icon Badge")
                                    checked: appController.showGlucoseInTrayIcon
                                    onToggled: appController.updateShowGlucoseInTrayIcon(checked)
                                    Layout.fillWidth: true
                                }

                                QQC2.CheckBox {
                                    text: qsTr("Tray Hover Info")
                                    checked: appController.showGlucoseInTrayTooltip
                                    onToggled: appController.updateShowGlucoseInTrayTooltip(checked)
                                    Layout.fillWidth: true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 4

                                QQC2.Label {
                                    text: qsTr("Theme:")
                                    color: Kirigami.Theme.textColor
                                }
                                QQC2.ToolButton {
                                    text: qsTr("System")
                                    checkable: true
                                    checked: appController.themeMode === "system"
                                    onClicked: appController.updateThemeMode("system")
                                }
                                QQC2.ToolButton {
                                    text: qsTr("Dark")
                                    checkable: true
                                    checked: appController.themeMode === "dark"
                                    onClicked: appController.updateThemeMode("dark")
                                }
                                QQC2.ToolButton {
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
                                QQC2.ToolButton {
                                    text: "mg/dL"
                                    checkable: true
                                    checked: appController.resultUnit === "mg/dL"
                                    onClicked: appController.updateResultUnit("mg/dL")
                                }
                                QQC2.ToolButton {
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
                    id: graphCard
                    objectName: "graphCard"
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: 14
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Kirigami.Heading {
                                level: 3
                                text: qsTr("Continuous Glucose Trend (%1)").arg(appController.resultUnit)
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                            }
                            QQC2.Label {
                                objectName: "graphTargetBandLabel"
                                text: glucoseCanvas.hoveredIndex >= 0
                                    ? (glucoseCanvas.hoveredValueText + " • " + glucoseCanvas.hoveredTimeText)
                                    : qsTr("Target band: %1").arg(appController.targetRangeDisplay)
                                font.bold: glucoseCanvas.hoveredIndex >= 0
                                opacity: glucoseCanvas.hoveredIndex >= 0 ? 0.95 : 0.75
                                color: Kirigami.Theme.textColor
                                elide: Text.ElideRight
                            }
                        }

                        Canvas {
                            id: glucoseCanvas
                            objectName: "glucoseCanvas"
                            Layout.fillWidth: true
                            Layout.fillHeight: true

                            property int hoveredIndex: -1
                            property real hoveredX: 0
                            property real hoveredY: 0
                            property string hoveredValueText: ""
                            property string hoveredStatusText: ""
                            property string hoveredTimeText: ""
                            property string hoveredColorHex: "#22c55e"

                            function clearHover() {
                                if (hoveredIndex !== -1) {
                                    hoveredIndex = -1
                                    hoveredValueText = ""
                                    hoveredStatusText = ""
                                    hoveredTimeText = ""
                                    requestPaint()
                                }
                            }

                            function updateHoverAt(mx, my) {
                                let pts = []
                                try {
                                    pts = JSON.parse(appController.graphPointsJson || "[]")
                                } catch (e) {
                                    pts = []
                                }
                                if (pts.length === 0 || width <= 0 || height <= 0) {
                                    clearHover()
                                    return
                                }

                                const padLeft = 48
                                const padRight = 20
                                const padTop = 16
                                const padBottom = 28
                                const plotW = Math.max(10, width - padLeft - padRight)
                                const plotH = Math.max(10, height - padTop - padBottom)

                                if (mx < padLeft - 16 || mx > width - padRight + 16 || my < 0 || my > height) {
                                    clearHover()
                                    return
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

                                // Find nearest point horizontally (with 2D distance tie-break)
                                let bestIdx = -1
                                let bestDist = 1e9
                                let bestX = 0
                                let bestY = 0
                                for (let i = 0; i < pts.length; ++i) {
                                    const v = Number(pts[i].value_mg_dl || 0)
                                    const px = pts.length === 1
                                        ? (padLeft + plotW / 2)
                                        : (padLeft + (plotW * i) / (pts.length - 1))
                                    const py = yForMg(v)
                                    const dx = Math.abs(mx - px)
                                    const dy = Math.abs(my - py)
                                    const score = dx + dy * 0.15
                                    if (score < bestDist) {
                                        bestDist = score
                                        bestIdx = i
                                        bestX = px
                                        bestY = py
                                    }
                                }

                                if (bestIdx < 0) {
                                    clearHover()
                                    return
                                }

                                const pt = pts[bestIdx]
                                const mg = Number(pt.value_mg_dl || 0)
                                const isMmol = appController.resultUnit === "mmol/L"
                                const formattedVal = isMmol
                                    ? (mg / 18.0).toFixed(1) + " mmol/L"
                                    : Math.round(mg).toString() + " mg/dL"

                                let statusLabel = qsTr("In Range")
                                let ptColor = "#22c55e"
                                if (mg < 70) {
                                    statusLabel = qsTr("Low")
                                    ptColor = "#ef4444"
                                } else if (mg > 240) {
                                    statusLabel = qsTr("High")
                                    ptColor = "#f97316"
                                } else if (mg < appController.targetLowMgDl || mg > appController.targetHighMgDl) {
                                    statusLabel = qsTr("Out of Target")
                                    ptColor = "#eab308"
                                }

                                hoveredIndex = bestIdx
                                hoveredX = bestX
                                hoveredY = bestY
                                hoveredValueText = formattedVal
                                hoveredStatusText = statusLabel
                                hoveredTimeText = String(pt.timestamp || qsTr("Unknown time"))
                                hoveredColorHex = ptColor
                                requestPaint()
                            }

                            onWidthChanged: {
                                clearHover()
                                requestPaint()
                            }
                            onHeightChanged: {
                                clearHover()
                                requestPaint()
                            }

                            HoverHandler {
                                id: canvasHoverHandler
                                onPointChanged: {
                                    if (hovered) {
                                        glucoseCanvas.updateHoverAt(point.position.x, point.position.y)
                                    }
                                }
                                onHoveredChanged: {
                                    if (!hovered) {
                                        glucoseCanvas.clearHover()
                                    }
                                }
                            }

                            MouseArea {
                                id: canvasHoverMouseArea
                                objectName: "glucoseCanvasMouseArea"
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.LeftButton
                                onPositionChanged: function(mouse) {
                                    glucoseCanvas.updateHoverAt(mouse.x, mouse.y)
                                }
                                onPressed: function(mouse) {
                                    glucoseCanvas.updateHoverAt(mouse.x, mouse.y)
                                }
                                onExited: {
                                    if (!canvasHoverHandler.hovered) {
                                        glucoseCanvas.clearHover()
                                    }
                                }
                            }

                            Rectangle {
                                id: graphHoverTooltip
                                objectName: "graphHoverTooltip"
                                visible: glucoseCanvas.hoveredIndex >= 0
                                z: 10
                                radius: 8
                                color: root.isLightTheme
                                    ? Qt.rgba(0.98, 0.99, 1.0, 0.96)
                                    : Qt.rgba(0.09, 0.12, 0.18, 0.95)
                                border.color: glucoseCanvas.hoveredColorHex
                                border.width: 1.5
                                width: Math.min(
                                    Math.max(120, Math.min(240, tooltipCol.implicitWidth + 20)),
                                    Math.max(120, glucoseCanvas.width - 16)
                                )
                                height: tooltipCol.implicitHeight + 14
                                implicitWidth: width
                                implicitHeight: height

                                x: {
                                    const preferredRight = glucoseCanvas.hoveredX + 12
                                    const preferredLeft = glucoseCanvas.hoveredX - width - 12
                                    const rawX = (preferredRight + width + 6 <= glucoseCanvas.width)
                                        ? preferredRight
                                        : preferredLeft
                                    return Math.max(6, Math.min(glucoseCanvas.width - width - 6, rawX))
                                }
                                y: {
                                    const preferredAbove = glucoseCanvas.hoveredY - height - 10
                                    const preferredBelow = glucoseCanvas.hoveredY + 12
                                    const rawY = (preferredAbove >= 6) ? preferredAbove : preferredBelow
                                    return Math.max(6, Math.min(glucoseCanvas.height - height - 6, rawY))
                                }

                                ColumnLayout {
                                    id: tooltipCol
                                    objectName: "graphHoverTooltipCol"
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.top: parent.top
                                    anchors.margins: 7
                                    spacing: 2

                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 6

                                        Rectangle {
                                            Layout.preferredWidth: 10
                                            Layout.preferredHeight: 10
                                            radius: 5
                                            color: glucoseCanvas.hoveredColorHex
                                        }

                                        QQC2.Label {
                                            objectName: "graphHoverValueLabel"
                                            text: glucoseCanvas.hoveredValueText
                                            font.bold: true
                                            font.pointSize: 10.5
                                            color: Kirigami.Theme.textColor
                                        }

                                        QQC2.Label {
                                            objectName: "graphHoverStatusLabel"
                                            text: "(" + glucoseCanvas.hoveredStatusText + ")"
                                            font.pointSize: 8.5
                                            font.bold: true
                                            color: glucoseCanvas.hoveredColorHex
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                        }
                                    }

                                    QQC2.Label {
                                        objectName: "graphHoverTimeLabel"
                                        text: glucoseCanvas.hoveredTimeText
                                        font.pointSize: 8.5
                                        opacity: 0.85
                                        color: Kirigami.Theme.textColor
                                        Layout.fillWidth: true
                                        elide: Text.ElideRight
                                    }
                                }
                            }

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
                                        "No CGM graph points received yet — click Refresh or Sample Data.",
                                        padLeft + 16,
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

                                // Vertical crosshair guide when hovering a point
                                if (hoveredIndex >= 0 && hoveredIndex < pts.length) {
                                    ctx.save()
                                    ctx.strokeStyle = root.isLightTheme
                                        ? "rgba(15, 23, 42, 0.35)"
                                        : "rgba(248, 250, 252, 0.35)"
                                    ctx.lineWidth = 1
                                    ctx.beginPath()
                                    ctx.moveTo(hoveredX, padTop)
                                    ctx.lineTo(hoveredX, padTop + plotH)
                                    ctx.stroke()
                                    ctx.restore()
                                }

                                // Plot points
                                for (let i = 0; i < pts.length; ++i) {
                                    const v = Number(pts[i].value_mg_dl || 0)
                                    const x = pts.length === 1
                                        ? (padLeft + plotW / 2)
                                        : (padLeft + (plotW * i) / (pts.length - 1))
                                    const y = yForMg(v)

                                    let ptColor = "#22c55e"
                                    if (v < 70) ptColor = "#ef4444"
                                    else if (v > 240) ptColor = "#f97316"
                                    else if (v < appController.targetLowMgDl || v > appController.targetHighMgDl) ptColor = "#eab308"

                                    if (i === hoveredIndex) {
                                        ctx.fillStyle = root.isLightTheme
                                            ? "rgba(2, 132, 199, 0.22)"
                                            : "rgba(56, 189, 248, 0.28)"
                                        ctx.beginPath()
                                        ctx.arc(x, y, 9, 0, Math.PI * 2)
                                        ctx.fill()

                                        ctx.fillStyle = ptColor
                                        ctx.beginPath()
                                        ctx.arc(x, y, 5.5, 0, Math.PI * 2)
                                        ctx.fill()

                                        ctx.strokeStyle = "#ffffff"
                                        ctx.lineWidth = 2
                                        ctx.stroke()
                                    } else {
                                        ctx.fillStyle = ptColor
                                        ctx.beginPath()
                                        ctx.arc(x, y, 3.5, 0, Math.PI * 2)
                                        ctx.fill()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ==================== TAB 2: GENERAL & TRAY SETTINGS ====================
        QQC2.ScrollView {
            id: generalScrollView
            objectName: "generalScrollView"
            implicitHeight: 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            visible: root.activeTab === "general"
            clip: true
            contentWidth: availableWidth

            ColumnLayout {
                id: generalContentColumn
                objectName: "generalContentColumn"
                width: Math.min(generalScrollView.availableWidth, 760)
                spacing: 14

                Kirigami.Heading {
                    level: 2
                    text: qsTr("General, Appearance & System Tray Settings")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    objectName: "traySettingsCard"
                    Layout.fillWidth: true
                    implicitHeight: trayGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: trayGroupCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 14
                        spacing: 10

                        Kirigami.Heading {
                            level: 3
                            text: qsTr("System Tray & Startup Behavior")
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                        }

                        QQC2.Switch {
                            text: qsTr("Show blood glucose level inside the System Tray icon badge")
                            checked: appController.showGlucoseInTrayIcon
                            onToggled: appController.updateShowGlucoseInTrayIcon(checked)
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            text: qsTr("Show blood glucose telemetry in the System Tray hover tooltip")
                            checked: appController.showGlucoseInTrayTooltip
                            onToggled: appController.updateShowGlucoseInTrayTooltip(checked)
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            text: qsTr("Launch automatically when logging into KDE Plasma")
                            checked: appController.autostartEnabled
                            onToggled: appController.updateAutostartEnabled(checked)
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            text: qsTr("Start minimized to the System Tray on login")
                            checked: appController.startMinimized
                            onToggled: appController.updateStartMinimized(checked)
                            Layout.fillWidth: true
                        }
                    }
                }

                Rectangle {
                    objectName: "displaySettingsCard"
                    Layout.fillWidth: true
                    implicitHeight: displayGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: displayGroupCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 14
                        spacing: 10

                        Kirigami.Heading {
                            level: 3
                            text: qsTr("Theme, Units & Desktop Overlay")
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            QQC2.Label {
                                text: qsTr("Color Theme:")
                                Layout.preferredWidth: 190
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 320
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
                            spacing: 12
                            QQC2.Label {
                                text: qsTr("Glucose Unit:")
                                Layout.preferredWidth: 190
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 320
                                model: ["mg/dL", "mmol/L"]
                                currentIndex: appController.resultUnit === "mmol/L" ? 1 : 0
                                onActivated: appController.updateResultUnit(currentText)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            QQC2.Label {
                                text: qsTr("Window Mode:")
                                Layout.preferredWidth: 190
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.ComboBox {
                                Layout.fillWidth: true
                                Layout.maximumWidth: 320
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

                        QQC2.Switch {
                            objectName: "overlayDarkThemeSwitch"
                            text: qsTr("Desktop Overlay Dark Theme (Dark slate background with status accent border)")
                            checked: appController.overlayDarkTheme
                            onToggled: appController.updateOverlayDarkTheme(checked)
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            objectName: "overlayAllDesktopsSwitch"
                            text: qsTr("Show Desktop Overlay on All Virtual Desktops")
                            checked: appController.overlayOnAllDesktops
                            onToggled: appController.updateOverlayOnAllDesktops(checked)
                            Layout.fillWidth: true
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            QQC2.Label {
                                text: qsTr("Desktop Overlay Opacity (%1%):").arg(appController.overlayOpacityPercent)
                                Layout.preferredWidth: 190
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.Slider {
                                id: overlayOpacitySlider
                                objectName: "overlayOpacitySlider"
                                Layout.fillWidth: true
                                Layout.maximumWidth: 220
                                from: 20
                                to: 100
                                stepSize: 5
                                value: appController.overlayOpacityPercent
                                onMoved: appController.updateOverlayOpacityPercent(Math.round(value))
                            }
                            QQC2.SpinBox {
                                id: overlayOpacitySpin
                                objectName: "overlayOpacitySpin"
                                from: 20
                                to: 100
                                stepSize: 5
                                value: appController.overlayOpacityPercent
                                onValueModified: appController.updateOverlayOpacityPercent(value)
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12
                            QQC2.Label {
                                text: qsTr("Polling Interval (seconds):")
                                Layout.preferredWidth: 190
                                color: Kirigami.Theme.textColor
                            }
                            QQC2.SpinBox {
                                from: 60
                                to: 3600
                                stepSize: 15
                                value: appController.pollIntervalSeconds
                                onValueModified: appController.updatePollIntervalSeconds(value)
                            }
                        }

                        QQC2.Switch {
                            objectName: "checkUpdatesSwitch"
                            text: qsTr("Automatically check GitHub Releases for updates (on startup & every 6 hours)")
                            checked: appController.checkUpdatesOnStartup
                            onToggled: appController.updateCheckUpdatesOnStartup(checked)
                            Layout.fillWidth: true
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            QQC2.Button {
                                objectName: "checkUpdatesNowButton"
                                icon.name: "system-software-update"
                                text: appController.checkingForUpdates ? qsTr("Checking...") : qsTr("Check for Updates")
                                enabled: !appController.checkingForUpdates
                                onClicked: appController.checkForUpdates()
                            }

                            QQC2.Label {
                                text: appController.updateStatusMessage
                                color: appController.updateAvailable ? "#0284c7" : Kirigami.Theme.textColor
                                font.bold: appController.updateAvailable
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                wrapMode: Text.WordWrap
                            }

                            QQC2.Button {
                                visible: appController.updateAvailable
                                icon.name: "download"
                                text: appController.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(appController.latestVersion)
                                enabled: !appController.installingUpdate
                                onClicked: appController.installUpdate()
                            }

                            QQC2.Button {
                                visible: appController.updateAvailable
                                icon.name: "globe"
                                text: qsTr("Release Notes")
                                onClicked: Qt.openUrlExternally(appController.updateUrl)
                            }
                        }
                    }
                }
            }
        }

        // ==================== TAB 3: ALERTS & THRESHOLDS ====================
        QQC2.ScrollView {
            id: alertsScrollView
            objectName: "alertsScrollView"
            implicitHeight: 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            visible: root.activeTab === "alerts"
            clip: true
            contentWidth: availableWidth

            ColumnLayout {
                id: alertsContentColumn
                objectName: "alertsContentColumn"
                width: Math.min(alertsScrollView.availableWidth, 760)
                spacing: 14

                Kirigami.Heading {
                    level: 2
                    text: qsTr("Glucose Alerts & Custom Thresholds")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    objectName: "alertsSettingsCard"
                    Layout.fillWidth: true
                    implicitHeight: alertsGroupCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: alertsGroupCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 14
                        spacing: 10

                        QQC2.Switch {
                            id: bringToFrontSwitch
                            text: qsTr("Bring main window to front when glucose is out of range")
                            checked: appController.bringToFrontEnabled
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            id: flashWindowSwitch
                            text: qsTr("Flash taskbar / window attention on out-of-range reading")
                            checked: appController.flashWindowEnabled
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            id: audioAlertSwitch
                            text: qsTr("Play audio alert when glucose is out of range")
                            checked: appController.audioAlertEnabled
                            Layout.fillWidth: true
                        }

                        QQC2.Switch {
                            id: customSoundSwitch
                            visible: audioAlertSwitch.checked
                            text: qsTr("Use custom audio file (.mp3 / .wav / .ogg)")
                            checked: appController.useCustomSound
                            Layout.fillWidth: true
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
                            Layout.fillWidth: true
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

                        Kirigami.Separator {
                            Layout.fillWidth: true
                        }

                        GridLayout {
                            Layout.fillWidth: true
                            columns: 2
                            columnSpacing: 18
                            rowSpacing: 6

                            QQC2.Label {
                                text: qsTr("Urgent Low Threshold (mg/dL)")
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                            }
                            QQC2.SpinBox {
                                id: urgentLowSpin
                                from: 40
                                to: 100
                                value: Math.round(appController.urgentLowMgDl)
                            }

                            QQC2.Label {
                                text: qsTr("Repeat alerts every (minutes)")
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                            }
                            QQC2.SpinBox {
                                id: alertRepeatSpin
                                from: 1
                                to: 240
                                value: appController.alertRepeatMinutes
                            }

                            QQC2.Switch {
                                id: alertOnStaleSwitch
                                text: qsTr("Alert when no new reading for (minutes)")
                                checked: appController.alertOnStale
                                Layout.fillWidth: true
                            }
                            QQC2.SpinBox {
                                id: staleAfterSpin
                                from: 5
                                to: 120
                                value: appController.staleAfterMinutes
                            }
                        }

                        QQC2.Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            opacity: 0.75
                            text: qsTr("Urgent low alerts repeat at least every 5 minutes and cannot be snoozed.")
                        }

                        RowLayout {
                            objectName: "snoozeRow"
                            Layout.fillWidth: true
                            spacing: 8

                            QQC2.Label {
                                text: appController.alertsSnoozed
                                      ? appController.snoozeUntilDisplay
                                      : qsTr("Snooze alerts:")
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            QQC2.Button {
                                text: qsTr("30 min")
                                icon.name: "notifications-disabled"
                                onClicked: appController.snoozeAlerts(30)
                            }
                            QQC2.Button {
                                text: qsTr("1 hour")
                                onClicked: appController.snoozeAlerts(60)
                            }
                            QQC2.Button {
                                text: qsTr("2 hours")
                                onClicked: appController.snoozeAlerts(120)
                            }
                            QQC2.Button {
                                visible: appController.alertsSnoozed
                                text: qsTr("Cancel")
                                icon.name: "notifications"
                                onClicked: appController.snoozeAlerts(0)
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
                                    appController.saveAdvancedAlertSettings(
                                        urgentLowSpin.value,
                                        alertRepeatSpin.value,
                                        staleAfterSpin.value,
                                        alertOnStaleSwitch.checked
                                    )
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
            id: accountScrollView
            objectName: "accountScrollView"
            implicitHeight: 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            visible: root.activeTab === "account"
            clip: true
            contentWidth: availableWidth

            ColumnLayout {
                id: accountContentColumn
                objectName: "accountContentColumn"
                width: Math.min(accountScrollView.availableWidth, 760)
                spacing: 14

                Kirigami.Heading {
                    level: 2
                    text: qsTr("Account & Encrypted Credential Vault")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    objectName: "accountSecurityCard"
                    Layout.fillWidth: true
                    implicitHeight: accountCol.implicitHeight + 28
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.15)
                    border.width: 1

                    ColumnLayout {
                        id: accountCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 14
                        spacing: 10

                        QQC2.Label {
                            text: qsTr("Session Status: %1").arg(appController.statusMessage)
                            font.bold: true
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            wrapMode: Text.WordWrap
                        }

                        QQC2.Label {
                            text: qsTr("Connected Patient: %1").arg(
                                appController.patientName.length > 0 ? appController.patientName : qsTr("None")
                            )
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10
                            QQC2.Label {
                                text: qsTr("Regional Endpoint: %1").arg(appController.country.toUpperCase())
                                color: Kirigami.Theme.textColor
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                wrapMode: Text.WordWrap
                            }
                            QQC2.ComboBox {
                                id: settingsRegionCombo
                                Layout.preferredWidth: 230
                                model: root.regionOptions
                                textRole: "label"
                                valueRole: "value"
                                currentIndex: root.regionIndexForCode(appController.country)
                                onActivated: function(index) {
                                    if (index >= 0 && index < root.regionOptions.length) {
                                        appController.updateCountry(root.regionOptions[index].value)
                                    }
                                }
                            }
                        }

                        QQC2.Label {
                            text: qsTr("Encryption Cipher: AES-256-GCM (PBKDF2-HMAC-SHA256, Secret Service / KWallet keyring + 0600 permissions)")
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            wrapMode: Text.WordWrap
                        }

                        QQC2.Switch {
                            objectName: "accountRememberPasswordSwitch"
                            text: qsTr("Remember password in encrypted vault for automatic token renewal")
                            checked: appController.rememberPassword
                            onToggled: appController.updateRememberPassword(checked)
                            Layout.fillWidth: true
                        }

                        QQC2.Label {
                            text: qsTr("Encrypted Vault Path: %1").arg(appController.credentialsVaultPath)
                            font.family: "monospace"
                            wrapMode: Text.WrapAnywhere
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                        }

                        RowLayout {
                            Layout.fillWidth: true
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
