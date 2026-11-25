import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.librelinkup 1.0

Kirigami.ApplicationWindow {
    id: root
    objectName: "librelinkupMainWindow"
    title: qsTr("LibreLinkUp Rust Desktop v%1 — KDE Plasma 6").arg(telemetry.appVersion)
    width: 980
    height: 680
    minimumWidth: 780
    minimumHeight: 540
    visible: false
    readonly property bool isLightTheme: (0.2126 * Kirigami.Theme.backgroundColor.r + 0.7152 * Kirigami.Theme.backgroundColor.g + 0.0722 * Kirigami.Theme.backgroundColor.b) > 0.45
    readonly property bool isHighContrast: Boolean(settings && settings.highContrastMode)
    readonly property real zoomScale: Math.max(0.75, Math.min(1.75, (settings ? settings.uiZoomPercent : 100) / 100.0))
    readonly property real osBaseFontPt: Kirigami.Theme.defaultFont.pointSize > 0 ? Kirigami.Theme.defaultFont.pointSize : 10.0
    readonly property real baseFontPt: osBaseFontPt * zoomScale
    readonly property real fontCaptionPt: Math.max(6.5, osBaseFontPt * 0.80 * zoomScale)
    readonly property real fontSmallPt: Math.max(7.0, osBaseFontPt * 0.85 * zoomScale)
    readonly property real fontCompactPt: Math.max(7.5, osBaseFontPt * 0.90 * zoomScale)
    readonly property real fontSubbodyPt: Math.max(8.0, osBaseFontPt * 0.95 * zoomScale)
    readonly property real fontBodyPt: Math.max(8.5, osBaseFontPt * 1.00 * zoomScale)
    readonly property real fontSubheadingPt: Math.max(9.0, osBaseFontPt * 1.05 * zoomScale)
    readonly property real fontHeadingPt: Math.max(9.5, osBaseFontPt * 1.15 * zoomScale)
    readonly property real fontTitlePt: Math.max(10.5, osBaseFontPt * 1.25 * zoomScale)
    readonly property real fontHeroTrendPt: Math.max(15.0, osBaseFontPt * 2.00 * zoomScale)
    readonly property real fontHeroValuePt: Math.max(25.0, osBaseFontPt * 3.40 * zoomScale)
    font.pointSize: baseFontPt

    onZoomScaleChanged: {
        fullHeaderWidth = 0
        Qt.callLater(recheckHeaderWidth)
    }
    readonly property color windowBgColor: isHighContrast
        ? (isLightTheme ? "#cbd5e1" : "#020617")
        : (isLightTheme ? "#dde5f0" : Kirigami.Theme.backgroundColor)
    readonly property color cardBgColor: isHighContrast
        ? (isLightTheme ? "#ffffff" : "#0f172a")
        : (isLightTheme ? "#ffffff" : Kirigami.Theme.alternateBackgroundColor)
    readonly property color subCardBgColor: isLightTheme ? "#edf2f7" : Qt.alpha(Kirigami.Theme.backgroundColor, 0.55)
    readonly property color cardBorderColor: isHighContrast
        ? (isLightTheme ? "#0f172a" : "#f8fafc")
        : (isLightTheme ? "#64748b" : Qt.alpha(Kirigami.Theme.textColor, 0.22))
    readonly property real cardBorderWidth: isHighContrast ? 2.5 : (isLightTheme ? 1.5 : 1.0)
    readonly property color subCardBorderColor: isHighContrast
        ? (isLightTheme ? "#1e293b" : "#cbd5e1")
        : (isLightTheme ? "#94a3b8" : Qt.alpha(Kirigami.Theme.textColor, 0.14))
    color: windowBgColor

    function statusBadgePrefix(label) {
        const clean = String(label || "").toLowerCase()
        if (clean.indexOf("urgent") !== -1 || clean === "low") return "⬇ "
        if (clean === "high") return "⬆ "
        if (clean.indexOf("out of") !== -1) return "⚠ "
        if (clean.indexOf("stale") !== -1) return "⏱ "
        if (clean.indexOf("in range") !== -1) return "✓ "
        return ""
    }

    function formatHaHost(rawUrl) {
        const u = String(rawUrl || "").trim()
        if (u.length === 0) return ""
        return u.replace(/^https?:\/\//i, "").replace(/\/.*$/, "")
    }

    readonly property bool isDemoSession: telemetry.loggedIn && telemetry.patientId === "demo-patient-001"
    readonly property string connectionSourceLabel: {
        if (root.isDemoSession) {
            return qsTr("Sample Data (Demo)")
        }
        if (settings.dataSource === "home_assistant") {
            const host = root.formatHaHost(settings.haUrl)
            return host.length > 0
                ? qsTr("Home Assistant (%1)").arg(host)
                : qsTr("Home Assistant")
        }
        return qsTr("LibreLinkUp (%1)").arg(settings.country.toUpperCase())
    }
    readonly property string connectionStateBadgeText: {
        if (telemetry.busy) {
            return qsTr("⟳ Syncing • %1").arg(root.connectionSourceLabel)
        }
        if (root.isDemoSession) {
            return qsTr("● Connected • %1").arg(root.connectionSourceLabel)
        }
        if (telemetry.loggedIn) {
            if (telemetry.errorMessage.length > 0 || telemetry.readingStale) {
                return qsTr("⚠ Connected • %1").arg(root.connectionSourceLabel)
            }
            return qsTr("● Connected • %1").arg(root.connectionSourceLabel)
        }
        return qsTr("○ Disconnected • %1").arg(root.connectionSourceLabel)
    }
    readonly property string connectionBadgeColor: {
        if (telemetry.busy) return "#eab308"
        if (root.isDemoSession) return "#0284c7"
        if (telemetry.loggedIn) {
            if (telemetry.errorMessage.length > 0 || telemetry.readingStale) return "#f59e0b"
            return "#22c55e"
        }
        if (telemetry.errorMessage.length > 0) return "#ef4444"
        return "#64748b"
    }
    readonly property string footerStatusSummaryText: {
        if (telemetry.loggedIn) {
            return telemetry.accessibleSummary
        }
        if (telemetry.errorMessage.length > 0) {
            return telemetry.errorMessage
        }
        return telemetry.statusMessage
    }

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
            if (headerRow.implicitWidth > 0) {
                fullHeaderWidth = Math.ceil(headerRow.implicitWidth)
            }
            const needed = fullHeaderWidth + headerSideMargins
            const maxWidth = Screen.desktopAvailableWidth > 0 ? Screen.desktopAvailableWidth - 40 : needed
            if (root.width < needed) {
                root.width = Math.min(needed, maxWidth)
            }
        })
    }

    function recheckHeaderWidth() {
        if (!root.headerCompact && headerRow.implicitWidth > 0) {
            const measured = Math.ceil(headerRow.implicitWidth)
            if (measured > root.fullHeaderWidth) {
                root.fullHeaderWidth = measured
            }
        } else if (root.fullHeaderWidth <= 0) {
            measureHeaderAndFitWindow()
        }
    }

    onVisibleChanged: {
        if (visible) {
            recheckHeaderWidth()
        }
    }

    Component.onCompleted: measureHeaderAndFitWindow()
    property string activeTab: "dashboard"
    readonly property alias controller: telemetry
    /// Alias kept for the overlay/dialogs that need both projections.
    readonly property alias settingsController: settings

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

    readonly property var haPatientsModel: {
        try {
            const parsed = JSON.parse(settings.haDiscoveredPatientsJson || "[]")
            return Array.isArray(parsed) ? parsed : []
        } catch (e) {
            return []
        }
    }

    readonly property var haPatientComboOptions: {
        const list = [{
            entity_id: "",
            patient_name: qsTr("Auto-discover"),
            label: qsTr("Auto-discover (First Active Sensor)"),
            compact_label: qsTr("Auto-discover (First Active Sensor)")
        }]
        for (let i = 0; i < root.haPatientsModel.length; ++i) {
            const p = root.haPatientsModel[i]
            const pname = String(p.patient_name || p.entity_id || "")
            const reading = String(p.reading_display || "")
            list.push({
                entity_id: String(p.entity_id || ""),
                patient_name: pname,
                label: String(p.label || pname),
                compact_label: reading.length > 0 ? (pname + " (" + reading + ")") : pname
            })
        }
        return list
    }

    function haPatientComboIndexForEntity(entityId) {
        const clean = String(entityId || "").trim().toLowerCase()
        if (clean.length === 0) return 0
        for (let i = 1; i < haPatientComboOptions.length; ++i) {
            if (String(haPatientComboOptions[i].entity_id || "").toLowerCase() === clean) {
                return i
            }
        }
        return 0
    }

    function openLogViewer() {
        root.activeTab = "logs"
        logViewerWindow.openLogWindow()
    }

    function openAboutDialog() {
        root.activeTab = "about"
        aboutDialogWindow.openAboutWindow()
    }

    property string editingMedId: ""

    onClosing: function(close) {
        close.accepted = false
        settings.hideMainWindow()
    }

    // ---------------------------------------------------------------------------------
    // Qt controllers. State lives in Rust (`bridge::state`); these three objects expose
    // disjoint slices of it:
    //   telemetry   - auth, CGM value/trend, analytics, alerts, tray/service host
    //   settings    - preferences, window/overlay placement, updater status, log viewer
    //   medications - medication schedule and dose log
    // ---------------------------------------------------------------------------------
    TelemetryController {
        id: telemetry
        objectName: "librelinkupAppController"
        Component.onCompleted: {
            settings.initialize()
            telemetry.initialize()
            medications.initialize()
        }
        onSharedStateChanged: {
            settings.sync()
            medications.sync()
        }
    }

    SettingsController {
        id: settings
        objectName: "librelinkupSettingsController"
        onUiZoomPercentChanged: root.recheckHeaderWidth()
        onPollIntervalSecondsChanged: {
            if (telemetry.loggedIn) {
                pollTimer.interval = telemetry.nextPollIntervalMs()
                pollTimer.restart()
            }
        }
    }

    MedicationsController {
        id: medications
        objectName: "librelinkupMedicationsController"
    }
    // Coarse staleness sweep (signal loss must be flagged even when fetches keep failing).
    Timer {
        id: stalenessTimer
        interval: 30000
        repeat: true
        running: telemetry.loggedIn
        onTriggered: telemetry.evaluateStaleness()
    }

    Timer {
        id: pollTimer
        // Jittered per cycle (+/-10%, computed in Rust) and re-armed on every tick.
        interval: telemetry.nextPollIntervalMs()
        repeat: false
        running: false
        onTriggered: {
            telemetry.refreshGlucoseNow()
            pollTimer.interval = telemetry.nextPollIntervalMs()
            pollTimer.restart()
        }
    }

    Connections {
        target: telemetry
        function onLoggedInChanged() {
            if (telemetry.loggedIn) {
                pollTimer.interval = telemetry.nextPollIntervalMs()
                pollTimer.restart()
            } else {
                pollTimer.stop()
            }
        }
    }

    // Periodically check GitHub Releases every 6 hours while running in the background
    Timer {
        id: updateCheckTimer
        interval: 6 * 3600 * 1000
        repeat: true
        running: settings.checkUpdatesOnStartup
        onTriggered: telemetry.checkForUpdatesSilent()
    }

    OverlayWidget {
        id: overlayWidgetWindow
        settings: settings
        telemetry: telemetry
    }


    header: QQC2.ToolBar {
        id: mainHeaderToolBar
        objectName: "mainHeaderToolBar"
        leftPadding: 0
        rightPadding: 0
        implicitHeight: headerRow.implicitHeight + 12

        RowLayout {
            id: headerRow
            objectName: "mainHeaderRow"
            anchors.fill: parent
            anchors.leftMargin: 6
            anchors.rightMargin: 6
            spacing: 1
            onImplicitWidthChanged: {
                if (!root.headerCompact && headerRow.implicitWidth > 0) {
                    const measured = Math.ceil(headerRow.implicitWidth)
                    if (measured > root.fullHeaderWidth) {
                        root.fullHeaderWidth = measured
                    }
                }
            }

            Rectangle {
                objectName: "vaultStatusBadge"
                radius: 6
                color: telemetry.credentialsEncrypted
                    ? Qt.rgba(0.13, 0.77, 0.37, 0.18)
                    : Qt.rgba(0.58, 0.64, 0.72, 0.18)
                border.color: telemetry.credentialsEncrypted ? "#22c55e" : "#94a3b8"
                border.width: 1
                implicitWidth: vaultLabel.implicitWidth + 10
                implicitHeight: 22
                Accessible.role: Accessible.StaticText
                Accessible.name: telemetry.credentialsEncrypted
                    ? qsTr("Credentials encrypted with AES-256-GCM")
                    : qsTr("Credentials vault unset")

                QQC2.Label {
                    id: vaultLabel
                    anchors.centerIn: parent
                    text: root.headerCompact
                        ? (telemetry.credentialsEncrypted ? "🔒" : "🔓")
                        : (telemetry.credentialsEncrypted ? qsTr("🔒 AES-256") : qsTr("🔓 Unset"))
                    font.pointSize: root.fontCaptionPt
                    font.bold: true
                    color: Kirigami.Theme.textColor
                }
            }

            Item { Layout.fillWidth: true }

            QQC2.ToolButton {
                objectName: "tabButtonDashboard"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: qsTr("Dashboard (Alt+1)")
                icon.name: "view-statistics"
                text: qsTr("Dashboard")
                Accessible.name: qsTr("Dashboard tab (Alt+1)")
                checkable: true
                checked: root.activeTab === "dashboard"
                onClicked: root.activeTab = "dashboard"
            }

            QQC2.ToolButton {
                objectName: "tabButtonMedications"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: qsTr("Medications (Alt+2)")
                icon.name: "bookmarks"
                text: qsTr("Medications")
                Accessible.name: qsTr("Medications and Glycemic Analytics tab (Alt+2)")
                checkable: true
                checked: root.activeTab === "medications"
                onClicked: root.activeTab = "medications"
            }

            QQC2.ToolButton {
                objectName: "tabButtonGeneral"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: qsTr("Settings (Alt+3)")
                icon.name: "configure"
                text: qsTr("Settings")
                Accessible.name: qsTr("General and Appearance Settings tab (Alt+3)")
                checkable: true
                checked: root.activeTab === "general"
                onClicked: root.activeTab = "general"
            }

            QQC2.ToolButton {
                objectName: "tabButtonAlerts"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: qsTr("Alerts (Alt+4)")
                icon.name: "preferences-desktop-notification-bell"
                text: qsTr("Alerts")
                Accessible.name: qsTr("Alerts and Thresholds tab (Alt+4)")
                checkable: true
                checked: root.activeTab === "alerts"
                onClicked: root.activeTab = "alerts"
            }

            QQC2.ToolButton {
                objectName: "tabButtonAccount"
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: root.headerCompact && hovered
                QQC2.ToolTip.text: qsTr("Account (Alt+5)")
                icon.name: "user-identity"
                text: qsTr("Account")
                Accessible.name: qsTr("Account and Encrypted Vault tab (Alt+5)")
                checkable: true
                checked: root.activeTab === "account"
                onClicked: root.activeTab = "account"
            }

            QQC2.ToolButton {
                objectName: "tabButtonLogs"
                icon.name: "utilities-log-viewer"
                text: qsTr("Logs (%1)").arg(settings.eventLogCount)
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Diagnostic Log Viewer (%1 entries, Alt+6)").arg(settings.eventLogCount)
                Accessible.name: qsTr("Diagnostic Log Viewer tab (%1 entries, Alt+6)").arg(settings.eventLogCount)
                checkable: true
                checked: root.activeTab === "logs"
                onClicked: root.openLogViewer()
            }

            QQC2.ToolButton {
                objectName: "tabButtonAbout"
                icon.name: "help-about"
                text: qsTr("About")
                display: root.headerCompact ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("About LibreLinkUp Rust Desktop v%1 (Alt+7)").arg(telemetry.appVersion)
                Accessible.name: qsTr("About LibreLinkUp Rust Desktop tab (Alt+7)")
                checkable: true
                checked: root.activeTab === "about"
                onClicked: root.openAboutDialog()
            }

            Rectangle {
                id: headerKofiBadge
                objectName: "headerKofiBadge"
                radius: 12
                color: headerKofiMouse.containsMouse || activeFocus ? "#c81e1e" : "#d93834"
                border.color: activeFocus ? "#ffffff" : "#ffffff33"
                border.width: activeFocus ? 2 : 1
                implicitWidth: headerKofiLabel.implicitWidth + 12
                implicitHeight: Math.max(24, Math.ceil(headerKofiLabel.implicitHeight + 6))
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Support Russell Milliner on Ko-fi")
                Accessible.description: qsTr("Opens https://ko-fi.com/russellmilliner in your web browser")
                Keys.onReturnPressed: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")
                Keys.onSpacePressed: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")

                QQC2.Label {
                    id: headerKofiLabel
                    anchors.centerIn: parent
                    text: root.headerCompact ? qsTr("☕") : qsTr("☕ Ko-fi")
                    font.bold: true
                    font.pointSize: root.fontSmallPt
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
                QQC2.ToolTip.text: qsTr("Refresh glucose telemetry now (F5 / Ctrl+R)")
                Accessible.name: qsTr("Refresh glucose telemetry now (F5 or Ctrl+R)")
                enabled: telemetry.loggedIn && !telemetry.busy
                onClicked: telemetry.refreshGlucoseNow()
            }
        }
    }

    footer: QQC2.ToolBar {
        id: mainFooterBar
        objectName: "mainFooterBar"
        leftPadding: 0
        rightPadding: 0
        implicitHeight: footerRow.implicitHeight + 8
        Accessible.role: Accessible.StatusBar
        Accessible.name: qsTr("Status and Accessibility Zoom Bar")

        RowLayout {
            id: footerRow
            objectName: "mainFooterRow"
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 8

            Rectangle {
                id: footerConnectionBadge
                objectName: "footerConnectionBadge"
                radius: 6
                color: Qt.alpha(Qt.color(root.connectionBadgeColor), root.isLightTheme ? 0.16 : 0.22)
                border.color: Qt.color(root.connectionBadgeColor)
                border.width: root.isHighContrast ? 2 : 1
                readonly property real maxBadgeW: Math.max(150, Math.round(root.width * 0.34))
                implicitWidth: Math.min(maxBadgeW, footerConnectionLabel.implicitWidth + 14)
                implicitHeight: Math.max(22, footerConnectionLabel.implicitHeight + 6)
                Layout.preferredWidth: implicitWidth
                Layout.maximumWidth: maxBadgeW
                Accessible.role: Accessible.StaticText
                Accessible.name: root.connectionStateBadgeText
                Accessible.description: telemetry.statusMessage

                QQC2.Label {
                    id: footerConnectionLabel
                    objectName: "footerConnectionLabel"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: 7
                    anchors.rightMargin: 7
                    text: root.connectionStateBadgeText
                    font.pointSize: root.fontCaptionPt
                    font.bold: true
                    color: Kirigami.Theme.textColor
                    elide: Text.ElideRight
                }

                HoverHandler { id: footerConnHover }
                QQC2.ToolTip.visible: footerConnHover.hovered
                QQC2.ToolTip.text: root.connectionStateBadgeText + " — " + telemetry.statusMessage
            }

            QQC2.Label {
                id: footerStatusLabel
                objectName: "footerStatusLabel"
                text: root.footerStatusSummaryText
                textFormat: Text.PlainText
                font.pointSize: Math.max(8.0, root.baseFontPt * 0.85)
                color: Kirigami.Theme.textColor
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                Accessible.role: Accessible.StatusBar
                Accessible.name: root.connectionStateBadgeText + " — " + text
                Accessible.description: qsTr("Live blood glucose telemetry, connection status, and accessibility status bar")
                HoverHandler { id: footerSummaryHover }
                QQC2.ToolTip.visible: footerSummaryHover.hovered
                QQC2.ToolTip.text: root.connectionStateBadgeText + " • " + root.footerStatusSummaryText
            }

            QQC2.ToolButton {
                id: footerHighContrastButton
                objectName: "footerHighContrastButton"
                display: QQC2.AbstractButton.TextOnly
                text: settings.highContrastMode ? qsTr("◑ High Contrast: ON") : qsTr("◑ High Contrast")
                font.pointSize: root.fontSmallPt
                font.bold: settings.highContrastMode
                leftPadding: 6
                rightPadding: 6
                checkable: true
                checked: settings.highContrastMode
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Toggle WCAG AAA High Contrast Accessibility Mode")
                Accessible.name: qsTr("Toggle High Contrast Accessibility Mode")
                onClicked: settings.updateHighContrastMode(!settings.highContrastMode)
            }

            QQC2.ToolButton {
                id: zoomOutButton
                objectName: "zoomOutButton"
                display: QQC2.AbstractButton.TextOnly
                text: "−"
                font.pointSize: root.fontBodyPt
                font.bold: true
                leftPadding: 8
                rightPadding: 8
                enabled: settings.uiZoomPercent > 75
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Zoom Out UI (Ctrl+-)")
                Accessible.name: qsTr("Zoom Out UI (Ctrl+-)")
                onClicked: settings.zoomOut()
            }

            QQC2.ToolButton {
                id: zoomResetButton
                objectName: "zoomResetButton"
                display: QQC2.AbstractButton.TextOnly
                text: settings.uiZoomPercent + "%"
                font.pointSize: root.fontSmallPt
                font.bold: true
                leftPadding: 6
                rightPadding: 6
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Reset UI Zoom to 100% (Ctrl+0)")
                Accessible.name: qsTr("Reset UI Zoom to 100 percent, current zoom %1 percent (Ctrl+0)").arg(settings.uiZoomPercent)
                onClicked: settings.resetZoom()
            }

            QQC2.ToolButton {
                id: zoomInButton
                objectName: "zoomInButton"
                display: QQC2.AbstractButton.TextOnly
                text: "+"
                font.pointSize: root.fontBodyPt
                font.bold: true
                leftPadding: 8
                rightPadding: 8
                enabled: settings.uiZoomPercent < 175
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: qsTr("Zoom In UI (Ctrl++)")
                Accessible.name: qsTr("Zoom In UI (Ctrl++)")
                onClicked: settings.zoomIn()
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
            visible: telemetry.errorMessage.length > 0
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
                    text: telemetry.errorMessage
                    textFormat: Text.PlainText
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
            visible: settings.updateAvailable
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
                    text: qsTr("New version available: %1 (current: v%2)").arg(settings.latestVersion).arg(telemetry.appVersion)
                    textFormat: Text.PlainText
                    font.bold: true
                    wrapMode: Text.WordWrap
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                QQC2.Button {
                    objectName: "bannerInstallUpdateButton"
                    icon.name: "download"
                    text: settings.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(settings.latestVersion)
                    enabled: !settings.installingUpdate
                    onClicked: telemetry.installUpdate()
                }

                QQC2.Button {
                    objectName: "bannerViewUpdateButton"
                    icon.name: "globe"
                    text: qsTr("View Release")
                    onClicked: Qt.openUrlExternally(settings.updateUrl)
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
            LoginView {
                id: loginScrollView
                anchors.fill: parent
                root: root
                telemetry: telemetry
                settings: settings
            }

            DashboardView {
                id: telemetryMainColumn
                anchors.fill: parent
                root: root
                telemetry: telemetry
                settings: settings
                medications: medications
            }
        }

        MedicationsView {
            id: medicationsScrollView
            root: root
            telemetry: telemetry
            medications: medications
        }

        SettingsView {
            id: generalScrollView
            root: root
            telemetry: telemetry
            settings: settings
        }

        AlertsView {
            id: alertsScrollView
            root: root
            telemetry: telemetry
            settings: settings
        }

        AccountView {
            id: accountScrollView
            root: root
            telemetry: telemetry
            settings: settings
            onSignedOut: {
                loginScrollView.clearPassword()
                root.activeTab = "dashboard"
            }
        }

        // ==================== TAB 6: DIAGNOSTIC LOGS ====================
        LogViewerDialog {
            id: logViewerWindow
            controller: settings
            visible: root.activeTab === "logs"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
        }

        // ==================== TAB 7: ABOUT ====================
        AboutDialog {
            id: aboutDialogWindow
            controller: settings
            telemetry: telemetry
            visible: root.activeTab === "about"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
        }
    }
}
