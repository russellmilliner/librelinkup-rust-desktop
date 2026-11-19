import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

Flickable {
    id: alertsScrollView

    required property var root
    required property var telemetry
    required property var settings

    function persistAlertSettings() {
        settings.saveAlertSettings(
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

    function persistAdvancedAlertSettings() {
        settings.saveAdvancedAlertSettings(
            urgentLowSpin.value,
            alertRepeatSpin.value,
            staleAfterSpin.value,
            alertOnStaleSwitch.checked
        )
    }

    FileDialog {
        id: customSoundFileDialog
        title: qsTr("Select Custom Alert Audio File")
        fileMode: FileDialog.OpenFile
        nameFilters: [qsTr("Audio Files (*.mp3 *.wav *.ogg *.flac)"), qsTr("All Files (*)")]
        onAccepted: {
            customSoundField.text = selectedFile.toString().replace("file://", "")
            alertsScrollView.persistAlertSettings()
        }
    }

    objectName: "alertsScrollView"
    implicitHeight: 0
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumHeight: 0
    Layout.preferredHeight: 0
    visible: root.activeTab === "alerts"
    clip: true
    contentWidth: width
    contentHeight: alertsContentColumn.implicitHeight + 16
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    QQC2.ScrollBar.vertical: HighContrastScrollBar {
        id: alertsVerticalScrollBar
        objectName: "alertsVerticalScrollBar"
        parent: alertsScrollView
        anchors.top: alertsScrollView.top
        anchors.right: alertsScrollView.right
        anchors.bottom: alertsScrollView.bottom
        policy: alertsScrollView.contentHeight > alertsScrollView.height
            ? QQC2.ScrollBar.AlwaysOn
            : QQC2.ScrollBar.AsNeeded
    }

    ColumnLayout {
        id: alertsContentColumn
        objectName: "alertsContentColumn"
        width: Math.min(Math.max(120, alertsScrollView.width - 18), Math.round(760 * Math.max(1.0, root.zoomScale)))
        spacing: 14

        Kirigami.Heading {
            level: 2
            font.pointSize: root.fontTitlePt
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
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

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
                    checked: settings.bringToFrontEnabled
                    onToggled: alertsScrollView.persistAlertSettings()
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    id: flashWindowSwitch
                    text: qsTr("Flash taskbar / window attention on out-of-range reading")
                    checked: settings.flashWindowEnabled
                    onToggled: alertsScrollView.persistAlertSettings()
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    id: audioAlertSwitch
                    text: qsTr("Play audio alert when glucose is out of range")
                    checked: settings.audioAlertEnabled
                    onToggled: alertsScrollView.persistAlertSettings()
                    Layout.fillWidth: true
                }

                QQC2.Switch {
                    id: customSoundSwitch
                    visible: audioAlertSwitch.checked
                    text: qsTr("Use custom audio file (.mp3 / .wav / .ogg)")
                    checked: settings.useCustomSound
                    onToggled: alertsScrollView.persistAlertSettings()
                    Layout.fillWidth: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: audioAlertSwitch.checked && customSoundSwitch.checked
                    spacing: 8

                    QQC2.TextField {
                        id: customSoundField
                        Layout.fillWidth: true
                        text: settings.customSoundPath
                        onEditingFinished: alertsScrollView.persistAlertSettings()
                        placeholderText: qsTr("/path/to/custom-alert.mp3")
                        Accessible.name: qsTr("Custom Alert Audio File Path")
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
                    checked: settings.overrideThreshold
                    onToggled: alertsScrollView.persistAlertSettings()
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
                            value: Math.round(settings.customTargetLow)
                            onValueModified: alertsScrollView.persistAlertSettings()
                            Accessible.name: qsTr("Custom Low Alert Threshold in mg/dL")
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
                            value: Math.round(settings.customTargetHigh)
                            onValueModified: alertsScrollView.persistAlertSettings()
                            Accessible.name: qsTr("Custom High Alert Threshold in mg/dL")
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
                        value: Math.round(settings.urgentLowMgDl)
                        onValueModified: alertsScrollView.persistAdvancedAlertSettings()
                        Accessible.name: qsTr("Urgent Low Alert Threshold in mg/dL")
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
                        value: settings.alertRepeatMinutes
                        onValueModified: alertsScrollView.persistAdvancedAlertSettings()
                        Accessible.name: qsTr("Repeat Alerts Interval in Minutes")
                    }

                    QQC2.Switch {
                        id: alertOnStaleSwitch
                        text: qsTr("Alert when no new reading for (minutes)")
                        checked: settings.alertOnStale
                        onToggled: alertsScrollView.persistAdvancedAlertSettings()
                        Layout.fillWidth: true
                    }
                    QQC2.SpinBox {
                        id: staleAfterSpin
                        from: 5
                        to: 120
                        value: settings.staleAfterMinutes
                        onValueModified: alertsScrollView.persistAdvancedAlertSettings()
                        Accessible.name: qsTr("Stale Sensor Reading Threshold in Minutes")
                    }

                    QQC2.Label {
                        text: qsTr("Sensor Expiry Warning (hours remaining, 0 = off)")
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                    }
                    QQC2.SpinBox {
                        id: sensorExpiryAlertSpin
                        objectName: "sensorExpiryAlertSpin"
                        from: 0
                        to: 168
                        value: settings.sensorExpiryAlertHours
                        onValueModified: settings.updateSensorExpiryAlertHours(value)
                        Accessible.name: qsTr("Sensor Expiry Warning Threshold in Hours")
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
                        text: telemetry.alertsSnoozed
                              ? telemetry.snoozeUntilDisplay
                              : qsTr("Snooze alerts:")
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    QQC2.Button {
                        text: qsTr("30 min")
                        icon.name: "notifications-disabled"
                        onClicked: telemetry.snoozeAlerts(30)
                    }
                    QQC2.Button {
                        text: qsTr("1 hour")
                        onClicked: telemetry.snoozeAlerts(60)
                    }
                    QQC2.Button {
                        text: qsTr("2 hours")
                        onClicked: telemetry.snoozeAlerts(120)
                    }
                    QQC2.Button {
                        visible: telemetry.alertsSnoozed
                        text: qsTr("Cancel")
                        icon.name: "notifications"
                        onClicked: telemetry.snoozeAlerts(0)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    QQC2.Button {
                        icon.name: "media-playback-start"
                        text: qsTr("Test Alert Now")
                        onClicked: settings.testAlertNow()
                    }
                }
            }
        }
    }
}
