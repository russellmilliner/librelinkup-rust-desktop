import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flickable {
    id: medicationsScrollView

    required property var root
    required property var telemetry
    required property var medications

    function scheduleIndexForCode(code) {
        const clean = String(code || "daily").toLowerCase()
        if (clean === "twice_daily") return 1
        if (clean === "weekly") return 2
        if (clean === "as_needed") return 3
        return 0
    }

    function startEditingMedication(item) {
        if (!item) return
        root.editingMedId = String(item.id || "")
        medNameField.text = String(item.name || "")
        medDosageField.text = String(item.dosage || "")
        medScheduleCombo.currentIndex = medicationsScrollView.scheduleIndexForCode(item.schedule_code)
        const wd = Number(item.weekday || 0)
        medWeekdayCombo.currentIndex = (wd >= 0 && wd <= 6) ? wd : 0
        medTimesField.text = String(item.scheduled_time || "08:00")
        medReminderSwitch.checked = Boolean(item.reminder_enabled)
    }

    function cancelEditingMedication() {
        root.editingMedId = ""
        medNameField.text = ""
        medDosageField.text = ""
        medScheduleCombo.currentIndex = 0
        medWeekdayCombo.currentIndex = 0
        medTimesField.text = "08:00"
        medReminderSwitch.checked = true
    }

    readonly property var medicationsListModel: {
        try {
            return JSON.parse(medications.medicationsJson || "[]")
        } catch (e) {
            return []
        }
    }

    readonly property var medicationDosesListModel: {
        try {
            return JSON.parse(medications.medicationDosesJson || "[]").slice(0, 25)
        } catch (e) {
            return []
        }
    }

    objectName: "medicationsScrollView"
    implicitHeight: 0
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumHeight: 0
    Layout.preferredHeight: 0
    visible: root.activeTab === "medications"
    clip: true
    contentWidth: width
    contentHeight: medicationsContentColumn.implicitHeight + 16
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    QQC2.ScrollBar.vertical: HighContrastScrollBar {
        id: medicationsVerticalScrollBar
        objectName: "medicationsVerticalScrollBar"
        parent: medicationsScrollView
        anchors.top: medicationsScrollView.top
        anchors.right: medicationsScrollView.right
        anchors.bottom: medicationsScrollView.bottom
        policy: medicationsScrollView.contentHeight > medicationsScrollView.height
            ? QQC2.ScrollBar.AlwaysOn
            : QQC2.ScrollBar.AsNeeded
    }

    ColumnLayout {
        id: medicationsContentColumn
        objectName: "medicationsContentColumn"
        width: Math.min(Math.max(120, medicationsScrollView.width - 18), Math.round(760 * Math.max(1.0, root.zoomScale)))
        spacing: 14

        Kirigami.Heading {
            level: 2
            font.pointSize: root.fontTitlePt
            text: qsTr("Medications, Dose Log & Estimated A1C")
            color: Kirigami.Theme.textColor
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
        }

        // Glycemic Analytics (eA1C / GMI / TIR%) Card
        Rectangle {
            objectName: "ea1cAnalyticsCard"
            Layout.fillWidth: true
            implicitHeight: ea1cCardCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: ea1cCardCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Kirigami.Heading {
                        level: 3
                        font.pointSize: root.fontHeadingPt
                        text: qsTr("Estimated A1C (eA1C) & Glycemic Management Indicator (GMI)")
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        wrapMode: Text.WordWrap
                    }

                    QQC2.ToolButton {
                        text: "12h"
                        display: QQC2.AbstractButton.TextOnly
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.statsWindow === "12h"
                        onClicked: telemetry.updateStatsWindow("12h")
                    }
                    QQC2.ToolButton {
                        text: "7d"
                        display: QQC2.AbstractButton.TextOnly
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.statsWindow === "7d"
                        onClicked: telemetry.updateStatsWindow("7d")
                    }
                    QQC2.ToolButton {
                        text: "14d"
                        display: QQC2.AbstractButton.TextOnly
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.statsWindow === "14d"
                        onClicked: telemetry.updateStatsWindow("14d")
                    }
                    QQC2.ToolButton {
                        text: "30d"
                        display: QQC2.AbstractButton.TextOnly
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.statsWindow === "30d"
                        onClicked: telemetry.updateStatsWindow("30d")
                    }
                    QQC2.ToolButton {
                        text: "90d"
                        display: QQC2.AbstractButton.TextOnly
                        leftPadding: 6
                        rightPadding: 6
                        checkable: true
                        checked: telemetry.statsWindow === "90d"
                        onClicked: telemetry.updateStatsWindow("90d")
                    }
                }

                QQC2.Label {
                    text: telemetry.ea1CHeadline
                    font.bold: true
                    font.pointSize: root.fontHeadingPt
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    text: telemetry.ea1CSubline
                    opacity: 0.85
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    text: qsTr("Formulas: ADAG eA1C (%) = (Mean mg/dL + 46.7) / 28.7 • Bergenstal GMI (%) = 3.31 + 0.02392 × Mean mg/dL.")
                    font.pointSize: root.fontSmallPt
                    opacity: 0.72
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }
            }
        }

        // Scheduled Medications & Quick Dose Checklist Card
        Rectangle {
            objectName: "medicationsScheduleCard"
            Layout.fillWidth: true
            implicitHeight: medsScheduleCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: medsScheduleCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Kirigami.Heading {
                        level: 3
                        font.pointSize: root.fontHeadingPt
                        text: qsTr("Scheduled Medications & Quick Dose Checklist")
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        wrapMode: Text.WordWrap
                    }

                    QQC2.Label {
                        text: medications.medicationsSummary
                        font.bold: true
                        opacity: 0.85
                        color: Kirigami.Theme.textColor
                    }
                }

                QQC2.Label {
                    visible: medicationsScrollView.medicationsListModel.length === 0
                    text: qsTr("No medications configured yet. Add your oral medications or insulin below, or load Sample Data on the Dashboard.")
                    wrapMode: Text.WordWrap
                    opacity: 0.78
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                Repeater {
                    model: medicationsScrollView.medicationsListModel

                    delegate: Rectangle {
                        readonly property bool isEditingThis: root.editingMedId === String(modelData.id || "") && root.editingMedId.length > 0
                        Layout.fillWidth: true
                        implicitHeight: medItemRow.implicitHeight + 16
                        radius: 8
                        color: isEditingThis
                            ? Qt.alpha(Kirigami.Theme.highlightColor, 0.14)
                            : root.subCardBgColor
                        border.color: isEditingThis
                            ? Kirigami.Theme.highlightColor
                            : root.subCardBorderColor
                        border.width: isEditingThis ? 1.5 : 1

                        RowLayout {
                            id: medItemRow
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 8
                            spacing: 8

                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                spacing: 2

                                QQC2.Label {
                                    text: String(modelData.name || "") + " — " + String(modelData.dosage || "")
                                    textFormat: Text.PlainText
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    wrapMode: Text.WordWrap
                                }

                                QQC2.Label {
                                    text: String(modelData.schedule_label || "")
                                        + (String(modelData.schedule_code) === "weekly" && modelData.weekday_label
                                            ? (" (" + String(modelData.weekday_label) + " " + String(modelData.scheduled_time || "") + ")")
                                            : (modelData.scheduled_time && String(modelData.schedule_code) !== "as_needed"
                                                ? (" (" + String(modelData.scheduled_time) + ")")
                                                : ""))
                                        + " • " + String(modelData.status_label || "")
                                    font.pointSize: root.fontSmallPt
                                    opacity: 0.82
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    wrapMode: Text.WordWrap
                                }
                            }

                            Rectangle {
                                readonly property string pillColor: {
                                    const c = String(modelData.status_code || "")
                                    if (c === "taken") return "#22c55e"
                                    if (c === "skipped") return "#64748b"
                                    if (c === "due") return "#f97316"
                                    if (c === "as_needed") return "#a855f7"
                                    return "#38bdf8"
                                }
                                radius: 6
                                color: Qt.alpha(pillColor, 0.18)
                                border.color: pillColor
                                border.width: 1
                                implicitWidth: medStatusLabel.implicitWidth + 12
                                implicitHeight: Math.max(22, medStatusLabel.implicitHeight + 6)

                                QQC2.Label {
                                    id: medStatusLabel
                                    anchors.centerIn: parent
                                    text: {
                                        const c = String(modelData.status_code || "")
                                        if (c === "taken") return qsTr("Taken")
                                        if (c === "skipped") return qsTr("Skipped")
                                        if (c === "due") return qsTr("Due Now")
                                        if (c === "as_needed") return qsTr("As Needed")
                                        return qsTr("Scheduled")
                                    }
                                    font.pointSize: root.fontCaptionPt
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                }
                            }

                            QQC2.Button {
                                objectName: "logMedicationDoseButton"
                                highlighted: true
                                icon.name: "dialog-ok-apply"
                                text: qsTr("Taken")
                                onClicked: medications.logMedicationDose(String(modelData.id || ""), "", "")
                            }

                            QQC2.Button {
                                objectName: "skipMedicationDoseButton"
                                visible: String(modelData.schedule_code || "") !== "as_needed"
                                icon.name: "media-skip-forward"
                                text: qsTr("Skipped")
                                onClicked: medications.skipMedicationDose(String(modelData.id || ""), "")
                            }

                            QQC2.ToolButton {
                                objectName: "editMedicationButton"
                                icon.name: "document-edit"
                                display: QQC2.AbstractButton.IconOnly
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.text: qsTr("Edit medication dosage or schedule")
                                Accessible.name: qsTr("Edit medication %1").arg(String(modelData.name || ""))
                                onClicked: medicationsScrollView.startEditingMedication(modelData)
                            }

                            QQC2.ToolButton {
                                icon.name: "edit-delete"
                                display: QQC2.AbstractButton.IconOnly
                                QQC2.ToolTip.visible: hovered
                                QQC2.ToolTip.text: qsTr("Remove medication")
                                Accessible.name: qsTr("Remove medication %1").arg(String(modelData.name || ""))
                                onClicked: {
                                    const delId = String(modelData.id || "")
                                    if (root.editingMedId === delId) {
                                        medicationsScrollView.cancelEditingMedication()
                                    }
                                    medications.removeMedication(delId)
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add / Update Medication Card
        Rectangle {
            objectName: "addMedicationCard"
            Layout.fillWidth: true
            implicitHeight: addMedCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: addMedCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                Kirigami.Heading {
                    level: 3
                    font.pointSize: root.fontHeadingPt
                    text: root.editingMedId.length > 0
                        ? qsTr("Edit Medication Dosage or Schedule")
                        : qsTr("Add Scheduled or As-Needed Medication")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Medication Name")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.TextField {
                            id: medNameField
                            objectName: "medNameField"
                            Layout.fillWidth: true
                            placeholderText: qsTr("e.g. Metformin ER, Lantus, Lispro")
                            Accessible.name: qsTr("Medication Name")
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Dosage")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.TextField {
                            id: medDosageField
                            objectName: "medDosageField"
                            Layout.fillWidth: true
                            placeholderText: qsTr("e.g. 500 mg, 18 units")
                            Accessible.name: qsTr("Medication Dosage")
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Schedule")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                        }
                        QQC2.ComboBox {
                            id: medScheduleCombo
                            objectName: "medScheduleCombo"
                            Layout.fillWidth: true
                            Accessible.name: qsTr("Medication Schedule")
                            model: [
                                { label: qsTr("Once Daily"), value: "daily" },
                                { label: qsTr("Twice Daily"), value: "twice_daily" },
                                { label: qsTr("Weekly"), value: "weekly" },
                                { label: qsTr("As Needed (PRN)"), value: "as_needed" }
                            ]
                            textRole: "label"
                            valueRole: "value"
                            onActivated: {
                                if (currentValue === "twice_daily" && medTimesField.text === "08:00") {
                                    medTimesField.text = "08:00, 20:00"
                                } else if (currentValue === "daily" && medTimesField.text === "08:00, 20:00") {
                                    medTimesField.text = "08:00"
                                }
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        visible: medScheduleCombo.currentValue === "weekly"
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Day of Week")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        QQC2.ComboBox {
                            id: medWeekdayCombo
                            objectName: "medWeekdayCombo"
                            Layout.fillWidth: true
                            Accessible.name: qsTr("Medication Day of Week")
                            model: [
                                { label: qsTr("Monday"), value: 0 },
                                { label: qsTr("Tuesday"), value: 1 },
                                { label: qsTr("Wednesday"), value: 2 },
                                { label: qsTr("Thursday"), value: 3 },
                                { label: qsTr("Friday"), value: 4 },
                                { label: qsTr("Saturday"), value: 5 },
                                { label: qsTr("Sunday"), value: 6 }
                            ]
                            textRole: "label"
                            valueRole: "value"
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        visible: medScheduleCombo.currentValue !== "as_needed"
                        spacing: 4
                        QQC2.Label {
                            text: qsTr("Scheduled Time(s) (24h HH:MM)")
                            font.bold: true
                            color: Kirigami.Theme.textColor
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                        QQC2.TextField {
                            id: medTimesField
                            objectName: "medTimesField"
                            Layout.fillWidth: true
                            text: "08:00"
                            placeholderText: qsTr("08:00 or 08:00, 20:00")
                            Accessible.name: qsTr("Scheduled Medication Times (24-hour HH:MM)")
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    QQC2.Switch {
                        id: medReminderSwitch
                        text: qsTr("Desktop reminder notification when due")
                        checked: true
                        Layout.fillWidth: true
                    }

                    QQC2.Button {
                        objectName: "cancelEditMedicationButton"
                        visible: root.editingMedId.length > 0
                        icon.name: "dialog-cancel"
                        text: qsTr("Cancel")
                        onClicked: medicationsScrollView.cancelEditingMedication()
                    }

                    QQC2.Button {
                        objectName: "addMedicationButton"
                        highlighted: true
                        icon.name: root.editingMedId.length > 0 ? "document-save" : "list-add"
                        text: root.editingMedId.length > 0
                            ? qsTr("Save Changes")
                            : qsTr("Add Medication")
                        enabled: medNameField.text.trim().length > 0
                        onClicked: {
                            medications.upsertMedication(
                                root.editingMedId,
                                medNameField.text,
                                medDosageField.text,
                                medScheduleCombo.currentValue,
                                medTimesField.text,
                                Number(medWeekdayCombo.currentValue || 0),
                                medReminderSwitch.checked,
                                ""
                            )
                            medicationsScrollView.cancelEditingMedication()
                        }
                    }
                }
            }
        }

        // Recent Dose History Log Card
        Rectangle {
            objectName: "doseHistoryCard"
            Layout.fillWidth: true
            implicitHeight: doseHistoryCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: doseHistoryCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 8

                Kirigami.Heading {
                    level: 3
                    font.pointSize: root.fontHeadingPt
                    text: qsTr("Recent Dose History (%1 logged)").arg(medications.medicationDosesCount)
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    visible: medicationsScrollView.medicationDosesListModel.length === 0
                    text: qsTr("No doses logged yet. Click 'Log Dose' above when taking a medication — doses within the last 12 hours are also plotted as purple markers (💊) on the Dashboard glucose trend graph.")
                    wrapMode: Text.WordWrap
                    opacity: 0.78
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                Repeater {
                    model: medicationsScrollView.medicationDosesListModel

                    delegate: RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        QQC2.Label {
                            text: (modelData.skipped ? "⏭ " : "💊 ")
                                + String(modelData.medication_name || "")
                                + " (" + String(modelData.dosage || "") + ")"
                                + (modelData.skipped ? (" — " + qsTr("Skipped")) : "")
                            textFormat: Text.PlainText
                            font.bold: true
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            elide: Text.ElideRight
                        }

                        QQC2.Label {
                            text: String(modelData.taken_at_display || "") + (modelData.glucose_mg_dl_at_dose ? (" • " + Math.round(Number(modelData.glucose_mg_dl_at_dose)) + " mg/dL") : "")
                            font.pointSize: root.fontSmallPt
                            opacity: 0.82
                            color: Kirigami.Theme.textColor
                            elide: Text.ElideRight
                        }

                        QQC2.ToolButton {
                            icon.name: "edit-delete"
                            display: QQC2.AbstractButton.IconOnly
                            QQC2.ToolTip.visible: hovered
                            QQC2.ToolTip.text: qsTr("Remove dose log entry")
                            Accessible.name: qsTr("Remove dose log entry for %1").arg(String(modelData.medication_name || ""))
                            onClicked: medications.removeMedicationDose(String(modelData.id || ""))
                        }
                    }
                }
            }
        }
    }
}
