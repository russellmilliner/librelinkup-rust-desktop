import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flickable {
    id: generalScrollView

    required property var root
    required property var telemetry
    required property var settings

    objectName: "generalScrollView"
    implicitHeight: 0
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumHeight: 0
    Layout.preferredHeight: 0
    visible: root.activeTab === "general"
    clip: true
    contentWidth: width
    contentHeight: generalContentColumn.implicitHeight + 16
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    QQC2.ScrollBar.vertical: HighContrastScrollBar {
        id: generalVerticalScrollBar
        objectName: "generalVerticalScrollBar"
        parent: generalScrollView
        anchors.top: generalScrollView.top
        anchors.right: generalScrollView.right
        anchors.bottom: generalScrollView.bottom
        policy: generalScrollView.contentHeight > generalScrollView.height
            ? QQC2.ScrollBar.AlwaysOn
            : QQC2.ScrollBar.AsNeeded
    }

    ColumnLayout {
        id: generalContentColumn
        objectName: "generalContentColumn"
        width: Math.min(Math.max(120, generalScrollView.width - 18), Math.round(760 * Math.max(1.0, root.zoomScale)))
        spacing: 14

        Kirigami.Heading {
            level: 2
            font.pointSize: root.fontTitlePt
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
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: trayGroupCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                Kirigami.Heading {
                    level: 3
                    font.pointSize: root.fontHeadingPt
                    text: qsTr("System Tray & Startup Behavior")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                }

                QQC2.Switch {
                    text: qsTr("Show blood glucose level inside the System Tray icon badge")
                    checked: settings.showGlucoseInTrayIcon
                    onToggled: settings.updateShowGlucoseInTrayIcon(checked)
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    text: qsTr("Show blood glucose telemetry in the System Tray hover tooltip")
                    checked: settings.showGlucoseInTrayTooltip
                    onToggled: settings.updateShowGlucoseInTrayTooltip(checked)
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    id: trayOnlyRelevantSwitch
                    objectName: "trayOnlyRelevantSwitch"
                    text: qsTr("Only mark tray icon as relevant when blood glucose is out of range")
                    checked: settings.trayOnlyRelevantOutOfRange
                    Accessible.name: qsTr("Only mark System Tray icon as relevant when blood glucose is out of range")
                    onToggled: settings.updateTrayOnlyRelevantOutOfRange(checked)
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    text: qsTr("Launch automatically when logging into KDE Plasma")
                    checked: settings.autostartEnabled
                    onToggled: settings.updateAutostartEnabled(checked)
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    text: qsTr("Start minimized to the System Tray on login")
                    checked: settings.startMinimized
                    onToggled: settings.updateStartMinimized(checked)
                    Layout.fillWidth: true
                }
            }
        }

        Rectangle {
            objectName: "displaySettingsCard"
            Layout.fillWidth: true
            implicitHeight: displayGroupCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: displayGroupCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                Kirigami.Heading {
                    level: 3
                    font.pointSize: root.fontHeadingPt
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
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        Layout.maximumWidth: 320
                        Accessible.name: qsTr("Color Theme")
                        model: [
                            { label: qsTr("System (Follow KDE Plasma)"), value: "system" },
                            { label: qsTr("Dark Mode (Breeze Dark)"), value: "dark" },
                            { label: qsTr("Light Mode (Breeze Light)"), value: "light" }
                        ]
                        textRole: "label"
                        valueRole: "value"
                        currentIndex: settings.themeMode === "dark" ? 1 : (settings.themeMode === "light" ? 2 : 0)
                        onActivated: settings.updateThemeMode(currentValue)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("UI Zoom Scale (%1%):").arg(settings.uiZoomPercent)
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.Button {
                        id: settingsZoomOutBtn
                        objectName: "settingsZoomOutBtn"
                        text: "−"
                        enabled: settings.uiZoomPercent > 75
                        Accessible.name: qsTr("Zoom Out UI (Ctrl+-)")
                        onClicked: settings.zoomOut()
                    }
                    QQC2.Button {
                        id: settingsZoomResetBtn
                        objectName: "settingsZoomResetBtn"
                        text: settings.uiZoomPercent + "% (100%)"
                        Accessible.name: qsTr("Reset UI Zoom to 100 percent (Ctrl+0)")
                        onClicked: settings.resetZoom()
                    }
                    QQC2.Button {
                        id: settingsZoomInBtn
                        objectName: "settingsZoomInBtn"
                        text: "+"
                        enabled: settings.uiZoomPercent < 175
                        Accessible.name: qsTr("Zoom In UI (Ctrl++)")
                        onClicked: settings.zoomIn()
                    }
                }

                QQC2.Switch {
                    id: highContrastSwitch
                    objectName: "highContrastSwitch"
                    text: qsTr("High Contrast Accessibility Mode (WCAG AAA reinforced borders & contrast)")
                    checked: settings.highContrastMode
                    onToggled: settings.updateHighContrastMode(checked)
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Glucose Unit:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        Layout.maximumWidth: 320
                        Accessible.name: qsTr("Glucose Unit")
                        model: ["mg/dL", "mmol/L"]
                        currentIndex: telemetry.resultUnit === "mmol/L" ? 1 : 0
                        onActivated: settings.updateResultUnit(currentText)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Window Mode:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        Layout.maximumWidth: 320
                        Accessible.name: qsTr("Window Mode")
                        model: [
                            { label: qsTr("Standard Windowed"), value: "windowed" },
                            { label: qsTr("Floating Desktop Overlay"), value: "overlay" },
                            { label: qsTr("Transparent Desktop Overlay"), value: "overlayTransparent" }
                        ]
                        textRole: "label"
                        valueRole: "value"
                        currentIndex: settings.windowMode === "overlay"
                            ? 1
                            : (settings.windowMode === "overlayTransparent" ? 2 : 0)
                        onActivated: settings.updateWindowMode(currentValue)
                    }
                }

                QQC2.Switch {
                    objectName: "overlayDarkThemeSwitch"
                    text: qsTr("Desktop Overlay Dark Theme (Dark slate background with status accent border)")
                    checked: settings.overlayDarkTheme
                    onToggled: settings.updateOverlayDarkTheme(checked)
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    objectName: "overlayAllDesktopsSwitch"
                    text: qsTr("Show Desktop Overlay on All Virtual Desktops")
                    checked: settings.overlayOnAllDesktops
                    onToggled: settings.updateOverlayOnAllDesktops(checked)
                    Layout.fillWidth: true
                }

                RowLayout {
                    objectName: "overlayOpacityRow"
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        objectName: "overlayOpacityLabel"
                        text: qsTr("Desktop Overlay Opacity (%1%):").arg(settings.overlayOpacityPercent)
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.Slider {
                        id: overlayOpacitySlider
                        objectName: "overlayOpacitySlider"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 110
                        Layout.maximumWidth: 220
                        from: 20
                        to: 100
                        stepSize: 5
                        value: settings.overlayOpacityPercent
                        Accessible.name: qsTr("Desktop Overlay Opacity Slider")
                        onMoved: settings.updateOverlayOpacityPercent(Math.round(value))
                    }
                    QQC2.SpinBox {
                        id: overlayOpacitySpin
                        objectName: "overlayOpacitySpin"
                        from: 20
                        to: 100
                        stepSize: 5
                        value: settings.overlayOpacityPercent
                        Accessible.name: qsTr("Desktop Overlay Opacity Percentage")
                        onValueModified: settings.updateOverlayOpacityPercent(value)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Polling Interval (seconds):")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.SpinBox {
                        from: 60
                        to: 3600
                        stepSize: 15
                        value: settings.pollIntervalSeconds
                        Accessible.name: qsTr("Polling Interval in Seconds")
                        onValueModified: settings.updatePollIntervalSeconds(value)
                    }
                }

                QQC2.Switch {
                    objectName: "checkUpdatesSwitch"
                    text: qsTr("Automatically check GitHub Releases for updates (on startup & every 6 hours)")
                    checked: settings.checkUpdatesOnStartup
                    onToggled: settings.updateCheckUpdatesOnStartup(checked)
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    QQC2.Button {
                        objectName: "checkUpdatesNowButton"
                        icon.name: "system-software-update"
                        text: settings.checkingForUpdates ? qsTr("Checking...") : qsTr("Check for Updates")
                        enabled: !settings.checkingForUpdates
                        onClicked: telemetry.checkForUpdates()
                    }

                    QQC2.Label {
                        text: settings.updateStatusMessage
                        textFormat: Text.PlainText
                        color: settings.updateAvailable ? "#0284c7" : Kirigami.Theme.textColor
                        font.bold: settings.updateAvailable
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        wrapMode: Text.WordWrap
                    }

                    QQC2.Button {
                        visible: settings.updateAvailable
                        icon.name: "download"
                        text: settings.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(settings.latestVersion)
                        enabled: !settings.installingUpdate
                        onClicked: telemetry.installUpdate()
                    }

                    QQC2.Button {
                        visible: settings.updateAvailable
                        icon.name: "globe"
                        text: qsTr("Release Notes")
                        onClicked: Qt.openUrlExternally(settings.updateUrl)
                    }
                }
            }
        }

        // Home Assistant LibreLink Integration Settings Card
        Rectangle {
            id: haIntegrationCard
            objectName: "haIntegrationCard"
            Layout.fillWidth: true
            implicitHeight: haIntegrationCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: haIntegrationCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                Kirigami.Heading {
                    level: 3
                    font.pointSize: root.fontHeadingPt
                    text: qsTr("Data Source & Home Assistant LibreLink Integration")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    text: qsTr("Choose whether to poll Abbott LibreLinkUp Cloud directly or pull CGM readings, trend arrows, and 12-hour history from your Home Assistant LibreLink integration.")
                    wrapMode: Text.WordWrap
                    opacity: 0.85
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Telemetry Source:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ComboBox {
                        id: settingsDataSourceCombo
                        objectName: "settingsDataSourceCombo"
                        Layout.fillWidth: true
                        Layout.maximumWidth: 360
                        Accessible.name: qsTr("Telemetry Data Source")
                        model: [
                            { label: qsTr("LibreLinkUp Cloud API"), value: "librelinkup" },
                            { label: qsTr("Home Assistant (LibreLink Integration)"), value: "home_assistant" }
                        ]
                        textRole: "label"
                        valueRole: "value"
                        currentIndex: settings.dataSource === "home_assistant" ? 1 : 0
                        onActivated: settings.updateDataSource(currentValue)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Home Assistant URL:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.TextField {
                        id: settingsHaUrlField
                        objectName: "settingsHaUrlField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("http://homeassistant.local:8123")
                        text: settings.haUrl
                        Accessible.name: qsTr("Home Assistant URL")
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Long-Lived Access Token:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.TextField {
                        id: settingsHaTokenField
                        objectName: "settingsHaTokenField"
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: settings.haTokenConfigured
                            ? qsTr("Encrypted in AES-256-GCM vault (leave blank to keep)")
                            : qsTr("Paste Home Assistant Long-Lived Access Token")
                        Accessible.name: qsTr("Home Assistant Long-Lived Access Token")
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Patient / Sensor:")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ComboBox {
                        id: settingsHaPatientCombo
                        objectName: "settingsHaPatientCombo"
                        Layout.fillWidth: true
                        model: root.haPatientComboOptions
                        textRole: "label"
                        valueRole: "entity_id"
                        currentIndex: root.haPatientComboIndexForEntity(settings.haSensorEntityId)
                        Accessible.name: qsTr("Home Assistant Patient or Sensor Selection")
                        onActivated: {
                            settingsHaEntityField.text = currentValue
                            loginHaEntityField.text = currentValue
                            telemetry.selectHaPatient(currentValue)
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    QQC2.Label {
                        text: qsTr("Sensor Entity ID (optional):")
                        wrapMode: Text.WordWrap
                        Layout.minimumWidth: Math.min(220, implicitWidth)
                        Layout.preferredWidth: Math.max(220, Math.min(280, implicitWidth))
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.TextField {
                        id: settingsHaEntityField
                        objectName: "settingsHaEntityField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Auto-discover (e.g. sensor.patient_glucose_measurement)")
                        text: settings.haSensorEntityId
                        Accessible.name: qsTr("Home Assistant Glucose Sensor Entity ID")
                    }
                }

                QQC2.Switch {
                    id: haSyncMedicationsSwitch
                    objectName: "haSyncMedicationsSwitch"
                    text: qsTr("Two-way Medication & Dose Sync with Home Assistant (sensor.librelinkup_medications)")
                    checked: settings.haSyncMedications
                    onToggled: settings.updateHaSyncMedications(checked)
                    Layout.fillWidth: true
                    Accessible.name: qsTr("Two-way Medication and Dose Sync with Home Assistant")
                }

                QQC2.Switch {
                    id: haSyncSnoozeSwitch
                    objectName: "haSyncSnoozeSwitch"
                    text: qsTr("Cross-Device Alert Snooze Synchronization via Home Assistant (sensor.librelinkup_alert_snooze)")
                    checked: settings.haSyncSnooze
                    onToggled: settings.updateHaSyncSnooze(checked)
                    Layout.fillWidth: true
                    Accessible.name: qsTr("Cross-Device Alert Snooze Synchronization with Home Assistant")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    QQC2.Button {
                        objectName: "saveHaConfigButton"
                        highlighted: true
                        icon.name: "network-connect"
                        text: telemetry.busy
                            ? qsTr("Connecting...")
                            : qsTr("Save & Connect Home Assistant")
                        enabled: !telemetry.busy
                        onClicked: {
                            telemetry.saveHomeAssistantConfig(
                                settingsDataSourceCombo.currentValue,
                                settingsHaUrlField.text,
                                settingsHaTokenField.text,
                                settingsHaEntityField.text
                            )
                            settingsHaTokenField.text = ""
                        }
                    }

                    QQC2.Button {
                        objectName: "autoFillHaEnvButton"
                        icon.name: "document-import"
                        text: qsTr("Import ~/.config/home-assistant.env")
                        onClicked: telemetry.autoFillHomeAssistantFromEnv()
                    }
                }
            }
        }
    }
}
