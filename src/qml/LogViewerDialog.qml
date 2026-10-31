import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami

Item {
    id: logViewerDialog
    objectName: "librelinkupLogViewerView"

    property var controller: null
    readonly property real zoomScale: Math.max(0.75, Math.min(1.75, (controller ? controller.uiZoomPercent : 100) / 100.0))
    readonly property real osBaseFontPt: Kirigami.Theme.defaultFont.pointSize > 0 ? Kirigami.Theme.defaultFont.pointSize : 10.0
    readonly property real fontSmallPt: osBaseFontPt * 0.85 * zoomScale
    readonly property real fontCompactPt: osBaseFontPt * 0.90 * zoomScale
    readonly property real fontBodyPt: osBaseFontPt * 1.00 * zoomScale
    property bool isLightTheme: (0.2126 * Kirigami.Theme.backgroundColor.r + 0.7152 * Kirigami.Theme.backgroundColor.g + 0.0722 * Kirigami.Theme.backgroundColor.b) > 0.45
    property string categoryFilter: "ALL"
    property string levelFilter: "ALL"
    property string searchQuery: ""

    readonly property var categories: [
        "ALL",
        "GLUCOSE",
        "MEDS",
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
            if (cat === "MEDS") return "#7e22ce"
            if (cat === "API") return "#0369a1"
            if (cat === "AUTH") return "#6d28d9"
            if (cat === "CRYPTO") return "#0f766e"
            if (cat === "TRAY") return "#4338ca"
            if (cat === "ALERT") return "#c2410c"
            return "#334155"
        }
        if (cat === "GLUCOSE") return "#4ade80"
        if (cat === "MEDS") return "#c084fc"
        if (cat === "API") return "#38bdf8"
        if (cat === "AUTH") return "#a78bfa"
        if (cat === "CRYPTO") return "#2dd4bf"
        if (cat === "TRAY") return "#818cf8"
        if (cat === "ALERT") return "#fb923c"
        return "#94a3b8"
    }

    component HighContrastScrollBar: QQC2.ScrollBar {
        id: hcScrollBar
        objectName: "highContrastScrollBar"
        policy: size < 1.0 ? QQC2.ScrollBar.AlwaysOn : QQC2.ScrollBar.AsNeeded
        interactive: true
        minimumSize: 0.14
        implicitWidth: orientation === Qt.Vertical ? 12 : (parent ? parent.width : 100)
        implicitHeight: orientation === Qt.Horizontal ? 12 : (parent ? parent.height : 100)
        x: orientation === Qt.Vertical ? (parent ? parent.width - width : 0) : 0
        y: orientation === Qt.Horizontal ? (parent ? parent.height - height : 0) : 0
        width: orientation === Qt.Vertical ? 12 : (parent ? parent.width : implicitWidth)
        height: orientation === Qt.Horizontal ? 12 : (parent ? parent.height : implicitHeight)
        z: 50
        padding: 2

        background: Rectangle {
            objectName: "highContrastScrollBarTrack"
            implicitWidth: hcScrollBar.orientation === Qt.Vertical ? 12 : 100
            implicitHeight: hcScrollBar.orientation === Qt.Horizontal ? 12 : 100
            radius: 6
            color: logViewerDialog.isLightTheme
                ? "#cbd5e1"
                : Qt.alpha(Kirigami.Theme.textColor, 0.16)
            border.width: 1
            border.color: logViewerDialog.isLightTheme
                ? "#475569"
                : Qt.alpha(Kirigami.Theme.textColor, 0.32)
            visible: hcScrollBar.size < 1.0 || hcScrollBar.policy === QQC2.ScrollBar.AlwaysOn
        }

        contentItem: Rectangle {
            objectName: "highContrastScrollBarThumb"
            implicitWidth: hcScrollBar.orientation === Qt.Vertical ? 8 : 48
            implicitHeight: hcScrollBar.orientation === Qt.Horizontal ? 8 : 48
            radius: 4
            visible: hcScrollBar.size < 1.0 || hcScrollBar.policy === QQC2.ScrollBar.AlwaysOn
            color: hcScrollBar.pressed
                ? Kirigami.Theme.highlightColor
                : (hcScrollBar.hovered
                    ? Qt.lighter(Kirigami.Theme.highlightColor, 1.15)
                    : (logViewerDialog.isLightTheme ? "#334155" : Qt.alpha(Kirigami.Theme.textColor, 0.68)))
            border.width: 1
            border.color: logViewerDialog.isLightTheme
                ? "#0f172a"
                : Qt.alpha(Kirigami.Theme.backgroundColor, 0.55)
        }
    }

    function refreshModel(forceScrollToEnd) {
        const prevCount = logListView.count
        const prevContentY = logListView.contentY
        const wasAtBottom = prevCount === 0
            || logListView.contentHeight <= logListView.height
            || logListView.atYEnd
            || (prevContentY >= logListView.contentHeight - logListView.height - 24)
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
                if (forceScrollToEnd || wasAtBottom) {
                    logListView.positionViewAtEnd()
                } else {
                    logListView.contentY = Math.min(
                        prevContentY,
                        Math.max(0, logListView.contentHeight - logListView.height)
                    )
                }
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
        refreshModel(true)
    }

    onVisibleChanged: {
        if (visible) {
            openLogWindow()
        }
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
        id: logMainColumn
        objectName: "logMainColumn"
        anchors.fill: parent
        spacing: 10

        RowLayout {
            id: logHeaderRow
            objectName: "logHeaderRow"
            Layout.fillWidth: true
            spacing: 8

            Rectangle {
                objectName: "logCountBadge"
                radius: 6
                color: Qt.alpha(Kirigami.Theme.highlightColor, 0.16)
                border.color: Qt.alpha(Kirigami.Theme.highlightColor, 0.45)
                border.width: 1
                readonly property real badgeW: countLabel.implicitWidth + 16
                readonly property real badgeH: Math.max(24, countLabel.implicitHeight + 8)
                implicitWidth: badgeW
                implicitHeight: badgeH
                Layout.preferredWidth: badgeW
                Layout.minimumWidth: badgeW
                Layout.preferredHeight: badgeH

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
                objectName: "logCopyButton"
                icon.name: "edit-copy"
                text: qsTr("Copy Shown")
                onClicked: logViewerDialog.copyText(logViewerDialog.formatFilteredAsText())
            }

            QQC2.Button {
                objectName: "logExportButton"
                icon.name: "document-save"
                text: qsTr("Export .log")
                onClicked: exportLogFileDialog.open()
            }

            QQC2.Button {
                objectName: "logClearButton"
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
            id: logFilterRow
            objectName: "logFilterRow"
            Layout.fillWidth: true
            spacing: 8

            QQC2.Label {
                text: qsTr("Category:")
                color: Kirigami.Theme.textColor
            }

            QQC2.ComboBox {
                id: categoryCombo
                objectName: "logCategoryCombo"
                model: logViewerDialog.categories
                Layout.preferredWidth: 130
                Layout.minimumWidth: 105
                Accessible.name: qsTr("Filter diagnostic logs by category")
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
                objectName: "logLevelCombo"
                model: logViewerDialog.levels
                Layout.preferredWidth: 100
                Layout.minimumWidth: 85
                Accessible.name: qsTr("Filter diagnostic logs by severity level")
                onActivated: {
                    logViewerDialog.levelFilter = currentText
                    logViewerDialog.refreshModel()
                }
            }

            QQC2.TextField {
                id: searchField
                objectName: "logSearchField"
                Layout.fillWidth: true
                Layout.minimumWidth: 120
                placeholderText: qsTr("Search timestamps, categories, or messages...")
                Accessible.name: qsTr("Search diagnostic log messages")
                onTextChanged: {
                    logViewerDialog.searchQuery = text
                    logViewerDialog.refreshModel()
                }
            }

            QQC2.Button {
                objectName: "logRefreshButton"
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
            objectName: "logListCard"
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: 8
            color: logViewerDialog.isLightTheme
                ? "#ffffff"
                : Qt.alpha(Kirigami.Theme.alternateBackgroundColor, 0.85)
            border.color: logViewerDialog.isLightTheme
                ? "#64748b"
                : Qt.alpha(Kirigami.Theme.textColor, 0.2)
            border.width: logViewerDialog.isLightTheme ? 1.5 : 1
            clip: true

            ListView {
                id: logListView
                objectName: "logListView"
                anchors.fill: parent
                anchors.margins: 6
                model: filteredLogModel
                spacing: 4
                clip: true
                cacheBuffer: 2000
                boundsBehavior: Flickable.StopAtBounds
                QQC2.ScrollBar.vertical: HighContrastScrollBar {
                    id: logVerticalScrollBar
                    objectName: "logVerticalScrollBar"
                    parent: logListView
                    anchors.top: logListView.top
                    anchors.right: logListView.right
                    anchors.bottom: logListView.bottom
                    policy: logListView.contentHeight > logListView.height
                        ? QQC2.ScrollBar.AlwaysOn
                        : QQC2.ScrollBar.AsNeeded
                }

                delegate: Rectangle {
                    id: logDelegateCard
                    objectName: "logDelegateCard"
                    width: Math.max(120, logListView.width - 18)
                    readonly property real contentRowHeight: Math.max(
                        metaRow.height,
                        messageLabel.contentHeight,
                        20
                    )
                    height: contentRowHeight + 12
                    implicitHeight: height
                    radius: 6
                    clip: true
                    color: logViewerDialog.isLightTheme
                        ? (index % 2 === 0 ? "#f1f5f9" : "#e2e8f0")
                        : (index % 2 === 0
                            ? Qt.alpha(Kirigami.Theme.backgroundColor, 0.65)
                            : Qt.alpha(Kirigami.Theme.alternateBackgroundColor, 0.45))
                    border.color: logViewerDialog.isLightTheme
                        ? "#94a3b8"
                        : Qt.alpha(Kirigami.Theme.textColor, 0.08)
                    border.width: 1

                    Row {
                        id: metaRow
                        objectName: "logMetaRow"
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.top: parent.top
                        anchors.topMargin: 6
                        height: Math.max(20, timestampLabel.implicitHeight + 4, levelBadge.height)
                        spacing: 8

                        QQC2.Label {
                            id: timestampLabel
                            objectName: "logTimestampLabel"
                            anchors.verticalCenter: parent.verticalCenter
                            text: model.timestamp
                            font.family: "monospace"
                            font.pointSize: logViewerDialog.fontCompactPt
                            opacity: 0.82
                            color: Kirigami.Theme.textColor
                            width: Math.ceil(implicitWidth) + 2
                        }

                        Rectangle {
                            id: levelBadge
                            objectName: "logLevelBadge"
                            anchors.verticalCenter: parent.verticalCenter
                            radius: 4
                            color: Qt.alpha(logViewerDialog.levelBadgeColor(model.level), 0.16)
                            border.color: logViewerDialog.levelBadgeColor(model.level)
                            border.width: 1
                            width: Math.max(54, Math.ceil(levelBadgeLabel.implicitWidth) + 12)
                            height: Math.max(20, Math.ceil(levelBadgeLabel.implicitHeight) + 6)
                            implicitWidth: width
                            implicitHeight: height

                            QQC2.Label {
                                id: levelBadgeLabel
                                anchors.centerIn: parent
                                text: model.level
                                font.bold: true
                                font.pointSize: logViewerDialog.fontSmallPt
                                color: logViewerDialog.levelBadgeColor(model.level)
                            }
                        }

                        Rectangle {
                            id: categoryBadge
                            objectName: "logCategoryBadge"
                            anchors.verticalCenter: parent.verticalCenter
                            radius: 4
                            color: Qt.alpha(logViewerDialog.categoryBadgeColor(model.category), 0.15)
                            border.color: logViewerDialog.categoryBadgeColor(model.category)
                            border.width: 1
                            width: Math.max(74, Math.ceil(categoryBadgeLabel.implicitWidth) + 12)
                            height: Math.max(20, Math.ceil(categoryBadgeLabel.implicitHeight) + 6)
                            implicitWidth: width
                            implicitHeight: height

                            QQC2.Label {
                                id: categoryBadgeLabel
                                anchors.centerIn: parent
                                text: model.category
                                font.bold: true
                                font.pointSize: logViewerDialog.fontSmallPt
                                color: logViewerDialog.categoryBadgeColor(model.category)
                            }
                        }
                    }

                    QQC2.Label {
                        id: messageLabel
                        objectName: "logMessageLabel"
                        anchors.left: metaRow.right
                        anchors.leftMargin: 10
                        anchors.right: parent.right
                        anchors.rightMargin: 8
                        anchors.top: parent.top
                        anchors.topMargin: 6
                        text: model.message
                        font.pointSize: logViewerDialog.fontBodyPt
                        wrapMode: Text.Wrap
                        color: Kirigami.Theme.textColor
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
    }
}
