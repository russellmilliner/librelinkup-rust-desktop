import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: aboutWindow
    objectName: "librelinkupAboutView"

    property var controller: null
    readonly property real zoomScale: Math.max(0.75, Math.min(1.75, (controller ? controller.uiZoomPercent : 100) / 100.0))
    readonly property real osBaseFontPt: Kirigami.Theme.defaultFont.pointSize > 0 ? Kirigami.Theme.defaultFont.pointSize : 10.0
    readonly property real fontSubbodyPt: osBaseFontPt * 0.95 * zoomScale
    readonly property real fontSubheadingPt: osBaseFontPt * 1.05 * zoomScale
    readonly property real fontTitlePt: osBaseFontPt * 1.25 * zoomScale
    readonly property bool isLightTheme: (0.2126 * Kirigami.Theme.backgroundColor.r + 0.7152 * Kirigami.Theme.backgroundColor.g + 0.0722 * Kirigami.Theme.backgroundColor.b) > 0.45
    property var telemetry: null
    readonly property string versionText: telemetry ? String(telemetry.appVersion || "0.4.5") : "0.4.5"

    function openAboutWindow() {
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
            color: aboutWindow.isLightTheme
                ? "#cbd5e1"
                : Qt.alpha(Kirigami.Theme.textColor, 0.16)
            border.width: 1
            border.color: aboutWindow.isLightTheme
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
                    : (aboutWindow.isLightTheme ? "#334155" : Qt.alpha(Kirigami.Theme.textColor, 0.68)))
            border.width: 1
            border.color: aboutWindow.isLightTheme
                ? "#0f172a"
                : Qt.alpha(Kirigami.Theme.backgroundColor, 0.55)
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        Flickable {
            id: aboutScroll
            objectName: "aboutScrollView"
            implicitHeight: 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            clip: true
            contentWidth: width
            contentHeight: aboutContentColumn.implicitHeight + 16
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.VerticalFlick
            QQC2.ScrollBar.vertical: HighContrastScrollBar {
                id: aboutVerticalScrollBar
                objectName: "aboutVerticalScrollBar"
                parent: aboutScroll
                anchors.top: aboutScroll.top
                anchors.right: aboutScroll.right
                anchors.bottom: aboutScroll.bottom
                policy: aboutScroll.contentHeight > aboutScroll.height
                    ? QQC2.ScrollBar.AlwaysOn
                    : QQC2.ScrollBar.AsNeeded
            }

            ColumnLayout {
                id: aboutContentColumn
                width: Math.min(Math.max(120, aboutScroll.width - 18), Math.round(760 * Math.max(1.0, aboutWindow.zoomScale)))
                spacing: 14

                Rectangle {
                    objectName: "aboutHeaderCard"
                    Layout.fillWidth: true
                    implicitHeight: headerCol.implicitHeight + 24
                    radius: 10
                    color: aboutWindow.isLightTheme ? "#ffffff" : Kirigami.Theme.alternateBackgroundColor
                    border.color: aboutWindow.isLightTheme ? "#64748b" : Qt.alpha(Kirigami.Theme.textColor, 0.16)
                    border.width: aboutWindow.isLightTheme ? 1.5 : 1

                    ColumnLayout {
                        id: headerCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                Kirigami.Heading {
                                    level: 2
                                    font.pointSize: aboutWindow.fontTitlePt
                                    text: qsTr("LibreLinkUp Rust Desktop")
                                    font.bold: true
                                    color: Kirigami.Theme.textColor
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    wrapMode: Text.WordWrap
                                }

                                QQC2.Label {
                                    text: qsTr("Version %1 • Native Rust + KDE Plasma 6 Kirigami").arg(aboutWindow.versionText)
                                    font.bold: true
                                    color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    wrapMode: Text.WordWrap
                                }
                            }

                            Rectangle {
                                id: dialogKofiBtn
                                objectName: "dialogKofiBtn"
                                radius: 15
                                color: dialogKofiMouse.containsMouse || activeFocus ? "#c81e1e" : "#d93834"
                                border.color: activeFocus ? "#ffffff" : "#ffffff33"
                                border.width: activeFocus ? 2 : 1
                                implicitWidth: dialogKofiRow.implicitWidth + 20
                                implicitHeight: Math.max(32, dialogKofiRow.implicitHeight + 10)
                                activeFocusOnTab: true
                                Accessible.role: Accessible.Button
                                Accessible.name: qsTr("Buy Russell Milliner a coffee on Ko-fi")
                                Keys.onReturnPressed: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")
                                Keys.onSpacePressed: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")

                                RowLayout {
                                    id: dialogKofiRow
                                    anchors.centerIn: parent
                                    spacing: 6

                                    QQC2.Label {
                                        text: "☕"
                                        font.pointSize: aboutWindow.fontSubheadingPt
                                    }

                                    QQC2.Label {
                                        text: qsTr("Buy Me a Coffee")
                                        font.bold: true
                                        font.pointSize: aboutWindow.fontSubbodyPt
                                        color: "#ffffff"
                                    }
                                }

                                MouseArea {
                                    id: dialogKofiMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")
                                }
                            }
                        }

                        QQC2.Label {
                            text: qsTr("Continuous blood glucose monitor for LibreLinkUp featuring AES-256-GCM encrypted vaults for credentials, 90-day glucose history (eA1C / GMI / TIR%) and medications, KDE system tray badge & tooltip, interactive 12-hour CGM chart, and a customizable desktop overlay. Contains zero tracking and zero advertisements — forever.")
                            color: Kirigami.Theme.textColor
                            opacity: 0.88
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            wrapMode: Text.WordWrap
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8

                            QQC2.Label {
                                text: settings ? settings.updateStatusMessage : ""
                                font.bold: settings ? settings.updateAvailable : false
                                color: (settings && settings.updateAvailable)
                                    ? (aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8")
                                    : Kirigami.Theme.textColor
                                opacity: 0.9
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                wrapMode: Text.WordWrap
                            }

                            QQC2.Button {
                                objectName: "aboutCheckUpdatesButton"
                                icon.name: "system-software-update"
                                text: (settings && settings.checkingForUpdates) ? qsTr("Checking...") : qsTr("Check Updates")
                                enabled: settings ? !settings.checkingForUpdates : false
                                onClicked: {
                                    if (telemetry) {
                                        telemetry.checkForUpdates()
                                    }
                                }
                            }

                            QQC2.Button {
                                visible: settings ? settings.updateAvailable : false
                                icon.name: "download"
                                text: settings ? (settings.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(settings.latestVersion)) : ""
                                enabled: settings ? !settings.installingUpdate : false
                                onClicked: {
                                    if (telemetry) {
                                        telemetry.installUpdate()
                                    }
                                }
                            }

                            QQC2.Button {
                                visible: settings ? settings.updateAvailable : false
                                icon.name: "globe"
                                text: qsTr("View Release")
                                onClicked: {
                                    if (settings && settings.updateUrl) {
                                        Qt.openUrlExternally(settings.updateUrl)
                                    }
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    objectName: "aboutLinksCard"
                    Layout.fillWidth: true
                    implicitHeight: linksCol.implicitHeight + 24
                    radius: 10
                    color: aboutWindow.isLightTheme ? "#ffffff" : Kirigami.Theme.alternateBackgroundColor
                    border.color: aboutWindow.isLightTheme ? "#64748b" : Qt.alpha(Kirigami.Theme.textColor, 0.16)
                    border.width: aboutWindow.isLightTheme ? 1.5 : 1

                    ColumnLayout {
                        id: linksCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 12
                        spacing: 8

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            QQC2.Label {
                                text: qsTr("Author:")
                                font.bold: true
                                color: Kirigami.Theme.textColor
                                Layout.minimumWidth: Math.max(140, implicitWidth)
                                Layout.preferredWidth: Math.max(140, implicitWidth)
                            }
                            QQC2.Label {
                                text: "Russell Milliner (https://github.com/russellmilliner)"
                                color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                font.underline: authorMouse.containsMouse
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                                MouseArea {
                                    id: authorMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://github.com/russellmilliner")
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            QQC2.Label {
                                text: qsTr("GitHub Repository:")
                                font.bold: true
                                color: Kirigami.Theme.textColor
                                Layout.minimumWidth: Math.max(140, implicitWidth)
                                Layout.preferredWidth: Math.max(140, implicitWidth)
                            }
                            QQC2.Label {
                                text: "https://github.com/russellmilliner/librelinkup-rust-desktop"
                                color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                font.underline: repoMouse.containsMouse
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                                MouseArea {
                                    id: repoMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://github.com/russellmilliner/librelinkup-rust-desktop")
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            QQC2.Label {
                                text: qsTr("Ko-fi Support:")
                                font.bold: true
                                color: Kirigami.Theme.textColor
                                Layout.minimumWidth: Math.max(140, implicitWidth)
                                Layout.preferredWidth: Math.max(140, implicitWidth)
                            }
                            QQC2.Label {
                                text: "https://ko-fi.com/russellmilliner"
                                color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                font.underline: kofiLinkMouse.containsMouse
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                                MouseArea {
                                    id: kofiLinkMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://ko-fi.com/russellmilliner")
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            QQC2.Label {
                                text: qsTr("License:")
                                font.bold: true
                                color: Kirigami.Theme.textColor
                                Layout.minimumWidth: Math.max(140, implicitWidth)
                                Layout.preferredWidth: Math.max(140, implicitWidth)
                            }
                            QQC2.Label {
                                text: "Apache License 2.0 (https://www.apache.org/licenses/LICENSE-2.0)"
                                color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                font.underline: licenseMouse.containsMouse
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                                MouseArea {
                                    id: licenseMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://www.apache.org/licenses/LICENSE-2.0")
                                }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            QQC2.Label {
                                text: qsTr("Attribution:")
                                font.bold: true
                                color: Kirigami.Theme.textColor
                                Layout.minimumWidth: Math.max(140, implicitWidth)
                                Layout.preferredWidth: Math.max(140, implicitWidth)
                            }
                            QQC2.Label {
                                text: "Based on Crazy-Marvin/LibreLinkUpDesktop (Apache-2.0)"
                                color: aboutWindow.isLightTheme ? "#0284c7" : "#38bdf8"
                                font.underline: upstreamMouse.containsMouse
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                elide: Text.ElideRight
                                MouseArea {
                                    id: upstreamMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: Qt.openUrlExternally("https://github.com/Crazy-Marvin/LibreLinkUpDesktop")
                                }
                            }
                        }
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            QQC2.Button {
                icon.name: "globe"
                text: qsTr("GitHub Repo")
                onClicked: Qt.openUrlExternally("https://github.com/russellmilliner/librelinkup-rust-desktop")
            }

            QQC2.Button {
                icon.name: "license"
                text: qsTr("Apache-2.0 License")
                onClicked: Qt.openUrlExternally("https://www.apache.org/licenses/LICENSE-2.0")
            }

            Item { Layout.fillWidth: true }
        }
    }
}
