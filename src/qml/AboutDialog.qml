import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: aboutWindow
    objectName: "librelinkupAboutView"

    property var controller: null
    readonly property bool isLightTheme: (0.2126 * Kirigami.Theme.backgroundColor.r + 0.7152 * Kirigami.Theme.backgroundColor.g + 0.0722 * Kirigami.Theme.backgroundColor.b) > 0.45
    property var telemetry: null
    readonly property string versionText: telemetry ? String(telemetry.appVersion || "0.4.5") : "0.4.5"

    function openAboutWindow() {
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        QQC2.ScrollView {
            id: aboutScroll
            implicitHeight: 0
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: 0
            clip: true
            contentWidth: availableWidth

            ColumnLayout {
                width: Math.min(aboutScroll.availableWidth, 760)
                spacing: 14

                Rectangle {
                    objectName: "aboutHeaderCard"
                    Layout.fillWidth: true
                    implicitHeight: headerCol.implicitHeight + 24
                    radius: 10
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.16)
                    border.width: 1

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
                                color: dialogKofiMouse.containsMouse ? "#c81e1e" : "#d93834"
                                border.color: "#ffffff33"
                                border.width: 1
                                implicitWidth: dialogKofiRow.implicitWidth + 20
                                implicitHeight: 32

                                RowLayout {
                                    id: dialogKofiRow
                                    anchors.centerIn: parent
                                    spacing: 6

                                    QQC2.Label {
                                        text: "☕"
                                        font.pointSize: 10.5
                                    }

                                    QQC2.Label {
                                        text: qsTr("Buy Me a Coffee")
                                        font.bold: true
                                        font.pointSize: 9.5
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
                    color: Kirigami.Theme.alternateBackgroundColor
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.16)
                    border.width: 1

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
                                Layout.preferredWidth: 130
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
                                Layout.preferredWidth: 130
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
                                Layout.preferredWidth: 130
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
                                Layout.preferredWidth: 130
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
                                Layout.preferredWidth: 130
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
