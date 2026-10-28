import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Window {
    id: aboutWindow
    objectName: "librelinkupAboutWindow"
    title: qsTr("About LibreLinkUp Rust Desktop")
    width: 580
    height: 500
    minimumWidth: 480
    minimumHeight: 400
    visible: false
    color: Kirigami.Theme.backgroundColor

    property var controller: null
    readonly property bool isLightTheme: (0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b) > 0.45
    readonly property string versionText: controller ? String(controller.appVersion || "0.4.5") : "0.4.5"

    function openAboutWindow() {
        aboutWindow.show()
        aboutWindow.raise()
        aboutWindow.requestActivate()
    }

    onClosing: function(close) {
        close.accepted = false
        aboutWindow.hide()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
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
                width: aboutScroll.availableWidth
                spacing: 10

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
                            text: qsTr("Continuous blood glucose monitor for LibreLinkUp featuring AES-256-GCM encrypted credentials, KDE system tray badge & tooltip, interactive 12-hour CGM chart, and a customizable desktop overlay.")
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
                                text: controller ? controller.updateStatusMessage : ""
                                font.bold: controller ? controller.updateAvailable : false
                                color: (controller && controller.updateAvailable)
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
                                text: (controller && controller.checkingForUpdates) ? qsTr("Checking...") : qsTr("Check Updates")
                                enabled: controller ? !controller.checkingForUpdates : false
                                onClicked: {
                                    if (controller) {
                                        controller.checkForUpdates()
                                    }
                                }
                            }

                            QQC2.Button {
                                visible: controller ? controller.updateAvailable : false
                                icon.name: "download"
                                text: controller ? (controller.installingUpdate ? qsTr("Installing...") : qsTr("Install %1").arg(controller.latestVersion)) : ""
                                enabled: controller ? !controller.installingUpdate : false
                                onClicked: {
                                    if (controller) {
                                        controller.installUpdate()
                                    }
                                }
                            }

                            QQC2.Button {
                                visible: controller ? controller.updateAvailable : false
                                icon.name: "globe"
                                text: qsTr("View Release")
                                onClicked: {
                                    if (controller && controller.updateUrl) {
                                        Qt.openUrlExternally(controller.updateUrl)
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

            QQC2.Button {
                objectName: "closeAboutButton"
                icon.name: "window-close"
                text: qsTr("Close")
                onClicked: aboutWindow.hide()
            }
        }
    }
}
