import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flickable {
    id: accountScrollView

    required property var root
    required property var telemetry
    required property var settings

    signal signedOut()

    objectName: "accountScrollView"
    implicitHeight: 0
    Layout.fillWidth: true
    Layout.fillHeight: true
    Layout.minimumHeight: 0
    Layout.preferredHeight: 0
    visible: root.activeTab === "account"
    clip: true
    contentWidth: width
    contentHeight: accountContentColumn.implicitHeight + 16
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    QQC2.ScrollBar.vertical: HighContrastScrollBar {
        id: accountVerticalScrollBar
        objectName: "accountVerticalScrollBar"
        parent: accountScrollView
        anchors.top: accountScrollView.top
        anchors.right: accountScrollView.right
        anchors.bottom: accountScrollView.bottom
        policy: accountScrollView.contentHeight > accountScrollView.height
            ? QQC2.ScrollBar.AlwaysOn
            : QQC2.ScrollBar.AsNeeded
    }

    ColumnLayout {
        id: accountContentColumn
        objectName: "accountContentColumn"
        width: Math.min(Math.max(120, accountScrollView.width - 18), Math.round(760 * Math.max(1.0, root.zoomScale)))
        spacing: 14

        Kirigami.Heading {
            level: 2
            font.pointSize: root.fontTitlePt
            text: qsTr("Account & Encrypted Credential Vault")
            color: Kirigami.Theme.textColor
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
        }

        Rectangle {
            objectName: "accountSecurityCard"
            Layout.fillWidth: true
            implicitHeight: accountCol.implicitHeight + 28
            radius: 10
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: accountCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 14
                spacing: 10

                QQC2.Label {
                    text: qsTr("Session Status: %1").arg(telemetry.statusMessage)
                    textFormat: Text.PlainText
                    font.bold: true
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Label {
                    text: qsTr("Connected Patient: %1").arg(
                        telemetry.patientName.length > 0 ? telemetry.patientName : qsTr("None")
                    )
                    textFormat: Text.PlainText
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10
                    QQC2.Label {
                        text: qsTr("Regional Endpoint: %1").arg(settings.country.toUpperCase())
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        wrapMode: Text.WordWrap
                    }
                    QQC2.ComboBox {
                        id: settingsRegionCombo
                        Layout.preferredWidth: 230
                        Accessible.name: qsTr("LibreLinkUp Regional Endpoint")
                        model: root.regionOptions
                        textRole: "label"
                        valueRole: "value"
                        currentIndex: root.regionIndexForCode(settings.country)
                        onActivated: function(index) {
                            if (index >= 0 && index < root.regionOptions.length) {
                                settings.updateCountry(root.regionOptions[index].value)
                            }
                        }
                    }
                }

                QQC2.Label {
                    text: qsTr("Encryption Cipher: AES-256-GCM (HKDF-SHA256 / PBKDF2-HMAC-SHA256, Secret Service / KWallet keyring + 0600 permissions)")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                QQC2.Switch {
                    objectName: "accountRememberPasswordSwitch"
                    text: qsTr("Remember password in encrypted vault for automatic token renewal")
                    checked: settings.rememberPassword
                    onToggled: settings.updateRememberPassword(checked)
                    Layout.fillWidth: true
                }

                QQC2.Label {
                    text: qsTr("Encrypted Vault Path: %1").arg(settings.credentialsVaultPath)
                    textFormat: Text.PlainText
                    font.family: "monospace"
                    wrapMode: Text.WrapAnywhere
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10
                    QQC2.Button {
                        icon.name: "system-log-out"
                        text: qsTr("Sign Out & Wipe Encrypted Vault")
                        enabled: telemetry.loggedIn || telemetry.credentialsEncrypted
                        onClicked: {
                            telemetry.logout()
                            accountScrollView.signedOut()
                        }
                    }
                }
            }
        }
    }
}
