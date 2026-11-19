import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: telemetryMainColumn

    required property var root
    required property var telemetry
    required property var settings
    required property var medications

    objectName: "telemetryMainColumn"
    anchors.fill: parent
    visible: telemetry.loggedIn
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
            Layout.preferredWidth: Math.round(270 * Math.min(1.25, root.zoomScale))
            Layout.minimumWidth: 250
            Layout.maximumWidth: Math.round(330 * Math.min(1.25, root.zoomScale))
            Layout.preferredHeight: topCardsRow.unifiedCardHeight
            implicitHeight: topCardsRow.unifiedCardHeight
            radius: 14
            color: root.isLightTheme
                ? Qt.darker(Qt.color(telemetry.rangeColorHex), 1.42)
                : Qt.darker(Qt.color(telemetry.rangeColorHex), 1.15)
            border.color: root.isHighContrast
                ? "#ffffff"
                : (root.isLightTheme
                    ? Qt.darker(Qt.color(telemetry.rangeColorHex), 1.95)
                    : Qt.rgba(1, 1, 1, 0.45))
            border.width: root.isHighContrast ? 2.5 : (root.isLightTheme ? 2.0 : 1.5)
            Accessible.role: Accessible.Pane
            Accessible.name: telemetry.accessibleSummary
            Accessible.description: telemetry.ea1CHeadline

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
                        font.pointSize: root.fontSmallPt
                        font.bold: true
                        color: Qt.rgba(1, 1, 1, 0.96)
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                    Rectangle {
                        radius: 6
                        color: Qt.rgba(0, 0, 0, 0.36)
                        border.color: Qt.rgba(1, 1, 1, 0.45)
                        border.width: 1
                        implicitWidth: statusPillLabel.implicitWidth + 12
                        implicitHeight: Math.max(20, statusPillLabel.implicitHeight + 6)
                        QQC2.Label {
                            id: statusPillLabel
                            objectName: "statusPillLabel"
                            anchors.centerIn: parent
                            text: root.statusBadgePrefix(telemetry.rangeStatusLabel) + telemetry.rangeStatusLabel
                            font.pointSize: root.fontCaptionPt
                            font.bold: true
                            color: "#ffffff"
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    QQC2.Label {
                        id: heroGlucoseValueLabel
                        objectName: "heroGlucoseValueLabel"
                        text: telemetry.glucoseDisplay
                        font.pointSize: root.fontHeroValuePt
                        font.bold: true
                        color: "#ffffff"
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        QQC2.Label {
                            text: telemetry.resultUnit
                            font.pointSize: root.fontHeadingPt
                            font.bold: true
                            color: "#ffffff"
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                        QQC2.Label {
                            text: telemetry.trendDescription
                            font.pointSize: root.fontCompactPt
                            color: Qt.rgba(1, 1, 1, 0.95)
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }

                    Rectangle {
                        Layout.preferredWidth: Math.round(44 * Math.min(1.25, root.zoomScale))
                        Layout.preferredHeight: Math.round(44 * Math.min(1.25, root.zoomScale))
                        radius: Math.round(22 * Math.min(1.25, root.zoomScale))
                        color: Qt.rgba(0, 0, 0, 0.34)
                        border.color: Qt.rgba(1, 1, 1, 0.85)
                        border.width: 1.5

                        QQC2.Label {
                            anchors.centerIn: parent
                            text: telemetry.trendSymbol
                            font.pointSize: root.fontHeroTrendPt
                            font.bold: true
                            color: "#ffffff"
                        }
                    }
                }

                QQC2.Label {
                    text: telemetry.readingStale
                          ? qsTr("⚠ STALE — updated: %1").arg(telemetry.lastUpdated)
                          : qsTr("Updated: %1").arg(telemetry.lastUpdated)
                    font.pointSize: root.fontSmallPt
                    font.bold: telemetry.readingStale
                    color: Qt.rgba(1, 1, 1, 0.94)
                    Layout.fillWidth: true
                    elide: Text.ElideRight
                }

                Rectangle {
                    objectName: "heroEa1cPill"
                    Layout.fillWidth: true
                    implicitHeight: Math.max(24, heroEa1cPillLabel.implicitHeight + 8)
                    radius: 6
                    color: Qt.rgba(0, 0, 0, 0.34)
                    border.color: Qt.rgba(1, 1, 1, 0.48)
                    border.width: 1

                    QQC2.Label {
                        id: heroEa1cPillLabel
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        text: qsTr("eA1C %1 • GMI %2 • TIR %3 (%4)")
                            .arg(telemetry.ea1CDisplay)
                            .arg(telemetry.gmiDisplay)
                            .arg(telemetry.tirDisplay)
                            .arg(telemetry.statsWindow)
                        font.pointSize: root.fontSmallPt
                        font.bold: true
                        color: "#ffffff"
                        elide: Text.ElideRight
                        horizontalAlignment: Text.AlignHCenter
                    }
                }

                QQC2.Label {
                    visible: telemetry.alertsSnoozed
                    text: "🔕 " + telemetry.snoozeUntilDisplay
                    font.pointSize: root.fontSmallPt
                    color: Qt.rgba(1, 1, 1, 0.95)
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
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth
            Accessible.role: Accessible.Pane
            Accessible.name: qsTr("CGM Sensor Overview and Quick Controls")
            Accessible.description: telemetry.ea1CHeadline

            ColumnLayout {
                id: detailsCol
                objectName: "detailsCol"
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 12
                spacing: 4

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Kirigami.Heading {
                        level: 3
                        font.pointSize: root.fontHeadingPt
                        text: telemetry.patientName.length > 0
                            ? qsTr("Patient: %1").arg(telemetry.patientName)
                            : qsTr("CGM Sensor Overview")
                        textFormat: Text.PlainText
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        elide: Text.ElideRight
                    }
                    QQC2.ToolButton {
                        icon.name: settings.overlayDarkTheme ? "weather-clear-night" : "color-picker"
                        text: qsTr("Overlay Dark")
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: settings.overlayDarkTheme
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: qsTr("Toggle dark slate background for the floating desktop overlay")
                        onClicked: settings.updateOverlayDarkTheme(!settings.overlayDarkTheme)
                    }
                    QQC2.ToolButton {
                        icon.name: "window-duplicate"
                        text: settings.overlayVisible
                            ? qsTr("Hide Overlay")
                            : qsTr("Overlay")
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        onClicked: settings.updateOverlayVisible(!settings.overlayVisible)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource === "home_assistant" && root.haPatientsModel.length > 1
                    spacing: 8

                    QQC2.Label {
                        text: qsTr("Active Patient:")
                        font.bold: true
                        font.pointSize: root.fontCompactPt
                        color: Kirigami.Theme.textColor
                    }

                    QQC2.ComboBox {
                        id: dashboardHaPatientCombo
                        objectName: "dashboardHaPatientCombo"
                        Layout.fillWidth: true
                        font.pointSize: root.fontCompactPt
                        model: root.haPatientComboOptions
                        textRole: "compact_label"
                        valueRole: "entity_id"
                        currentIndex: root.haPatientComboIndexForEntity(settings.haSensorEntityId)
                        Accessible.name: qsTr("Select Home Assistant Patient")
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
                        text: qsTr("Target: %1").arg(telemetry.targetRangeDisplay)
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        elide: Text.ElideRight
                    }

                    QQC2.Label {
                        text: settings.dataSource === "home_assistant"
                            ? qsTr("Source: Home Assistant • Poll: %1s").arg(settings.pollIntervalSeconds)
                            : qsTr("Region: %1 • Poll: %2s").arg(settings.country.toUpperCase()).arg(settings.pollIntervalSeconds)
                        color: Kirigami.Theme.textColor
                        elide: Text.ElideRight
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    QQC2.Label {
                        id: sensorStatusValueLabel
                        objectName: "sensorStatusValueLabel"
                        text: qsTr("Sensor: %1").arg(telemetry.sensorStatusDisplay)
                        textFormat: Text.PlainText
                        font.pointSize: root.fontCompactPt
                        font.bold: telemetry.sensorInWarmup || telemetry.sensorExpiringSoon
                        color: (telemetry.sensorInWarmup || telemetry.sensorExpiringSoon)
                            ? Kirigami.Theme.neutralTextColor
                            : Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        elide: Text.ElideRight
                        Accessible.role: Accessible.StaticText
                        Accessible.name: qsTr("CGM Sensor Status: %1").arg(telemetry.sensorStatusDisplay)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    QQC2.CheckBox {
                        text: qsTr("Tray Icon Badge")
                        checked: settings.showGlucoseInTrayIcon
                        onToggled: settings.updateShowGlucoseInTrayIcon(checked)
                        Layout.fillWidth: true
                    }

                    QQC2.CheckBox {
                        text: qsTr("Tray Hover Info")
                        checked: settings.showGlucoseInTrayTooltip
                        onToggled: settings.updateShowGlucoseInTrayTooltip(checked)
                        Layout.fillWidth: true
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    QQC2.Label {
                        objectName: "dashboardEa1cHeadlineLabel"
                        text: telemetry.ea1CHeadline
                        textFormat: Text.PlainText
                        font.bold: true
                        font.pointSize: root.fontSubbodyPt
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        elide: Text.ElideRight
                        QQC2.ToolTip.visible: ea1cHover.hovered
                        QQC2.ToolTip.text: telemetry.ea1CSubline
                        HoverHandler { id: ea1cHover }
                    }

                    QQC2.ToolButton {
                        text: "12h"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSmallPt
                        leftPadding: 4
                        rightPadding: 4
                        checkable: true
                        checked: telemetry.statsWindow === "12h"
                        onClicked: telemetry.updateStatsWindow("12h")
                    }
                    QQC2.ToolButton {
                        text: "7d"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSmallPt
                        leftPadding: 4
                        rightPadding: 4
                        checkable: true
                        checked: telemetry.statsWindow === "7d"
                        onClicked: telemetry.updateStatsWindow("7d")
                    }
                    QQC2.ToolButton {
                        text: "14d"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSmallPt
                        leftPadding: 4
                        rightPadding: 4
                        checkable: true
                        checked: telemetry.statsWindow === "14d"
                        onClicked: telemetry.updateStatsWindow("14d")
                    }
                    QQC2.ToolButton {
                        text: "30d"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSmallPt
                        leftPadding: 4
                        rightPadding: 4
                        checkable: true
                        checked: telemetry.statsWindow === "30d"
                        onClicked: telemetry.updateStatsWindow("30d")
                    }
                    QQC2.ToolButton {
                        text: "90d"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSmallPt
                        leftPadding: 4
                        rightPadding: 4
                        checkable: true
                        checked: telemetry.statsWindow === "90d"
                        onClicked: telemetry.updateStatsWindow("90d")
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    QQC2.Label {
                        text: qsTr("Theme:")
                        font.pointSize: root.fontSubbodyPt
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ToolButton {
                        text: qsTr("System")
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: settings.themeMode === "system"
                        onClicked: settings.updateThemeMode("system")
                    }
                    QQC2.ToolButton {
                        text: qsTr("Dark")
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: settings.themeMode === "dark"
                        onClicked: settings.updateThemeMode("dark")
                    }
                    QQC2.ToolButton {
                        text: qsTr("Light")
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: settings.themeMode === "light"
                        onClicked: settings.updateThemeMode("light")
                    }

                    Item { Layout.fillWidth: true }

                    QQC2.Label {
                        text: qsTr("Unit:")
                        font.pointSize: root.fontSubbodyPt
                        color: Kirigami.Theme.textColor
                    }
                    QQC2.ToolButton {
                        text: "mg/dL"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.resultUnit === "mg/dL"
                        onClicked: settings.updateResultUnit("mg/dL")
                    }
                    QQC2.ToolButton {
                        text: "mmol/L"
                        display: QQC2.AbstractButton.TextOnly
                        font.pointSize: root.fontSubbodyPt
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.resultUnit === "mmol/L"
                        onClicked: settings.updateResultUnit("mmol/L")
                    }
                }
            }
        }
    }

    // 12-Hour Glucose Trend Graph Card
    // 12-Hour Glucose Trend Graph Card
    GlucoseChart {
        id: glucoseChart
        root: telemetryMainColumn.root
        telemetry: telemetryMainColumn.telemetry
        settings: telemetryMainColumn.settings
        medications: telemetryMainColumn.medications
    }
}
