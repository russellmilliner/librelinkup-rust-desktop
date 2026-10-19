import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

Window {
    id: logViewerDialog
    objectName: "librelinkupLogViewerWindow"
    title: qsTr("LibreLinkUp Desktop — Diagnostic Log Viewer")
    width: 880
    height: 560
    minimumWidth: 680
    minimumHeight: 420
    visible: false
    color: Kirigami.Theme.backgroundColor

    property var controller: null
    property bool isLightTheme: (0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b) > 0.45
    property string categoryFilter: "ALL"
    property string levelFilter: "ALL"
    property string searchQuery: ""

    readonly property var categories: [
        "ALL",
        "GLUCOSE",
        "API",
        "AUTH",
        "CRYPTO",
        "TRAY",
        "ALERT",
        "SYSTEM"
    ]
    readonly property var levels: ["ALL", "INFO", "WARN", "ERROR"]

    ListModel {
        id: filteredLogModel
    }

    TextEdit {
        id: logClipboardHelper
        visible: false
    }

    function copyText(txt) {
        logClipboardHelper.text = String(txt || "")
        logClipboardHelper.selectAll()
        logClipboardHelper.copy()
        logClipboardHelper.deselect()
    }

    function levelBadgeColor(lvl) {
        if (lvl === "ERROR") return isLightTheme ? "#b91c1c" : "#f87171"
        if (lvl === "WARN") return isLightTheme ? "#b45309" : "#fbbf24"
        return isLightTheme ? "#0369a1" : "#38bdf8"
    }

    function categoryBadgeColor(cat) {
        if (isLightTheme) {
            if (cat === "GLUCOSE") return "#15803d"
            if (cat === "API") return "#0369a1"
            if (cat === "AUTH") return "#6d28d9"
            if (cat === "CRYPTO") return "#0f766e"
            if (cat === "TRAY") return "#4338ca"
            if (cat === "ALERT") return "#c2410c"
            return "#334155"
        }
        if (cat === "GLUCOSE") return "#4ade80"
        if (cat === "API") return "#38bdf8"
        if (cat === "AUTH") return "#a78bfa"
        if (cat === "CRYPTO") return "#2dd4bf"
        if (cat === "TRAY") return "#818cf8"
        if (cat === "ALERT") return "#fb923c"
        return "#94a3b8"
    }

    function refreshModel() {
        filteredLogModel.clear()
        if (!controller) return
        const raw = controller.eventLogJson || "[]"
        const cat = (logViewerDialog.categoryFilter || "ALL").toUpperCase()
        const lvl = (logViewerDialog.levelFilter || "ALL").toUpperCase()
        const q = (logViewerDialog.searchQuery || "").trim().toLowerCase()
        try {
            const arr = JSON.parse(raw)
            if (!Array.isArray(arr)) return
            for (let i = 0; i < arr.length; ++i) {
                const item = arr[i]
                const itemLvl = String(item.level || "INFO").toUpperCase()
                const itemCat = String(item.category || "SYSTEM").toUpperCase()
                const itemMsg = String(item.message || "")
                const itemTs = String(item.timestamp || "")
                if (cat !== "ALL" && itemCat !== cat) continue
                if (lvl !== "ALL" && itemLvl !== lvl) continue
                if (q.length > 0) {
                    const hay = (itemTs + " " + itemLvl + " " + itemCat + " " + itemMsg).toLowerCase()
                    if (hay.indexOf(q) === -1) continue
                }
                filteredLogModel.append({
                    timestamp: itemTs,
                    level: itemLvl,
                    category: itemCat,
                    message: itemMsg
                })
            }
            if (logListView.count > 0) {
                logListView.positionViewAtEnd()
            }
        } catch (e) {}
    }

    function formatFilteredAsText() {
        let lines = []
        for (let i = 0; i < filteredLogModel.count; ++i) {
            const it = filteredLogModel.get(i)
            lines.push("[" + it.timestamp + "] [" + it.level + "] [" + it.category + "] " + it.message)
        }
        return lines.join("\n")
    }

    function openLogWindow() {
        if (controller) {
            controller.refreshEventLogs()
        }
        refreshModel()
        logViewerDialog.showNormal()
        logViewerDialog.show()
        logViewerDialog.raise()
        logViewerDialog.requestActivate()
    }

    Connections {
        target: logViewerDialog.controller
        function onEventLogJsonChanged() {
            if (logViewerDialog.visible) {
                logViewerDialog.refreshModel()
            }
        }
    }

    FileDialog {
        id: exportLogFileDialog
        title: qsTr("Export Diagnostic Log (.log)")
        fileMode: FileDialog.SaveFile
        defaultSuffix: "log"
        nameFilters: [qsTr("Log Files (*.log *.txt)"), qsTr("All Files (*)")]
        onAccepted: {
            if (logViewerDialog.controller) {
                logViewerDialog.controller.exportEventLogs(selectedFile.toString())
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.mediumSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.mediumSpacing

            Kirigami.Heading {
                level: 2
                text: qsTr("Diagnostic Event & API Log")
                color: Kirigami.Theme.textColor
            }

            Rectangle {
                radius: 6
                color: Qt.alpha(Kirigami.Theme.highlightColor, 0.16)
                border.color: Qt.alpha(Kirigami.Theme.highlightColor, 0.45)
                border.width: 1
                implicitWidth: countLabel.implicitWidth + 16
                implicitHeight: 24

                QQC2.Label {
                    id: countLabel
                    anchors.centerIn: parent
                    text: qsTr("%1 entries").arg(filteredLogModel.count)
                    font.bold: true
                    color: Kirigami.Theme.textColor
                }
            }

            Item { Layout.fillWidth: true }

            QQC2.Button {
                icon.name: "edit-copy"
                text: qsTr("Copy Shown")
                onClicked: logViewerDialog.copyText(logViewerDialog.formatFilteredAsText())
            }

            QQC2.Button {
                icon.name: "document-save"
                text: qsTr("Export .log")
                onClicked: exportLogFileDialog.open()
            }

            QQC2.Button {
                icon.name: "edit-clear-history"
                text: qsTr("Clear")
                onClicked: {
                    if (logViewerDialog.controller) {
                        logViewerDialog.controller.clearEventLogs()
                        logViewerDialog.refreshModel()
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            QQC2.Label {
                text: qsTr("Category:")
                color: Kirigami.Theme.textColor
            }

            QQC2.ComboBox {
                id: categoryCombo
                model: logViewerDialog.categories
                Layout.preferredWidth: 140
                onActivated: {
                    logViewerDialog.categoryFilter = currentText
                    logViewerDialog.refreshModel()
                }
            }

            QQC2.Label {
                text: qsTr("Level:")
                color: Kirigami.Theme.textColor
            }

            QQC2.ComboBox {
                id: levelCombo
                model: logViewerDialog.levels
                Layout.preferredWidth: 110
                onActivated: {
                    logViewerDialog.levelFilter = currentText
                    logViewerDialog.refreshModel()
                }
            }

            QQC2.TextField {
                id: searchField
                Layout.fillWidth: true
                placeholderText: qsTr("Search timestamps, categories, or messages...")
                onTextChanged: {
                    logViewerDialog.searchQuery = text
                    logViewerDialog.refreshModel()
                }
            }

            QQC2.Button {
                icon.name: "view-refresh"
                text: qsTr("Refresh")
                onClicked: {
                    if (logViewerDialog.controller) {
                        logViewerDialog.controller.refreshEventLogs()
                    }
                    logViewerDialog.refreshModel()
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: 8
            color: Qt.alpha(Kirigami.Theme.alternateBackgroundColor, 0.85)
            border.color: Qt.alpha(Kirigami.Theme.textColor, 0.2)
            border.width: 1
            clip: true

            ListView {
                id: logListView
                anchors.fill: parent
                anchors.margins: 6
                model: filteredLogModel
                spacing: 4
                clip: true
                QQC2.ScrollBar.vertical: QQC2.ScrollBar {
                    policy: QQC2.ScrollBar.AsNeeded
                }

                delegate: Rectangle {
                    width: logListView.width - 14
                    implicitHeight: rowLayout.implicitHeight + 10
                    radius: 6
                    color: index % 2 === 0
                        ? Qt.alpha(Kirigami.Theme.backgroundColor, 0.65)
                        : Qt.alpha(Kirigami.Theme.alternateBackgroundColor, 0.45)
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.08)
                    border.width: 1

                    RowLayout {
                        id: rowLayout
                        anchors.fill: parent
                        anchors.margins: 5
                        spacing: 8

                        QQC2.Label {
                            text: model.timestamp
                            font.family: "monospace"
                            opacity: 0.78
                            color: Kirigami.Theme.textColor
                            Layout.preferredWidth: 165
                        }

                        Rectangle {
                            radius: 4
                            color: Qt.alpha(logViewerDialog.levelBadgeColor(model.level), 0.16)
                            border.color: logViewerDialog.levelBadgeColor(model.level)
                            border.width: 1
                            Layout.preferredWidth: 58
                            implicitHeight: 20

                            QQC2.Label {
                                anchors.centerIn: parent
                                text: model.level
                                font.bold: true
                                font.pointSize: 8.5
                                color: logViewerDialog.levelBadgeColor(model.level)
                            }
                        }

                        Rectangle {
                            radius: 4
                            color: Qt.alpha(logViewerDialog.categoryBadgeColor(model.category), 0.15)
                            border.color: logViewerDialog.categoryBadgeColor(model.category)
                            border.width: 1
                            Layout.preferredWidth: 78
                            implicitHeight: 20

                            QQC2.Label {
                                anchors.centerIn: parent
                                text: model.category
                                font.bold: true
                                font.pointSize: 8.5
                                color: logViewerDialog.categoryBadgeColor(model.category)
                            }
                        }

                        QQC2.Label {
                            text: model.message
                            wrapMode: Text.WrapAnywhere
                            color: Kirigami.Theme.textColor
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            QQC2.Label {
                anchors.centerIn: parent
                visible: filteredLogModel.count === 0
                text: qsTr("No matching log events.")
                opacity: 0.65
                color: Kirigami.Theme.textColor
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            QQC2.Button {
                icon.name: "window-close"
                text: qsTr("Close")
                onClicked: logViewerDialog.close()
            }
        }
    }
}
