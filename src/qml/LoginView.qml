import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flickable {
    id: loginScrollView

    required property var root
    required property var telemetry
    required property var settings

    function clearPassword() {
        loginPasswordField.text = ""
    }

    objectName: "loginScrollView"
    anchors.fill: parent
    visible: !telemetry.loggedIn
    clip: true
    contentWidth: width
    contentHeight: Math.max(loginScrollView.height, loginCard.implicitHeight + 24)
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    QQC2.ScrollBar.vertical: HighContrastScrollBar {
        id: loginVerticalScrollBar
        objectName: "loginVerticalScrollBar"
        parent: loginScrollView
        anchors.top: loginScrollView.top
        anchors.right: loginScrollView.right
        anchors.bottom: loginScrollView.bottom
        policy: loginScrollView.contentHeight > loginScrollView.height
            ? QQC2.ScrollBar.AlwaysOn
            : QQC2.ScrollBar.AsNeeded
    }

    Item {
        width: Math.max(120, loginScrollView.width - 16)
        height: Math.max(loginScrollView.height, loginCard.implicitHeight + 24)
        implicitHeight: loginCard.implicitHeight + 24

        Rectangle {
            id: loginCard
            objectName: "loginCard"
            anchors.centerIn: parent
            width: Math.min(parent.width - 24, 510)
            implicitHeight: loginCol.implicitHeight + 36
            radius: 12
            color: root.cardBgColor
            border.color: root.cardBorderColor
            border.width: root.cardBorderWidth

            ColumnLayout {
                id: loginCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                spacing: 12

                Kirigami.Heading {
                    level: 2
                    font.pointSize: root.fontTitlePt
                    text: settings.dataSource === "home_assistant"
                        ? qsTr("Connect to Home Assistant LibreLink")
                        : qsTr("Sign In to LibreLinkUp")
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    wrapMode: Text.WordWrap
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("CGM Data Source")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.ComboBox {
                        id: loginDataSourceCombo
                        objectName: "loginDataSourceCombo"
                        Layout.fillWidth: true
                        Accessible.name: qsTr("CGM Data Source")
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

                QQC2.Label {
                    visible: settings.dataSource !== "home_assistant"
                    text: qsTr("Use your LibreLinkUp follower credentials. All credentials and tokens are encrypted at rest using AES-256-GCM + PBKDF2-HMAC-SHA256.")
                    wrapMode: Text.WordWrap
                    opacity: 0.85
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource !== "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Email / Username")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.TextField {
                        id: loginEmailField
                        Layout.fillWidth: true
                        placeholderText: qsTr("you@example.com")
                        text: telemetry.username
                        Accessible.name: qsTr("LibreLinkUp Email or Username")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource !== "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Password")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.TextField {
                        id: loginPasswordField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Enter your LibreLinkUp password")
                        Accessible.name: qsTr("LibreLinkUp Password")
                        onAccepted: signInButton.clicked()
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource !== "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Regional Server")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.ComboBox {
                        id: loginRegionCombo
                        Layout.fillWidth: true
                        Accessible.name: qsTr("LibreLinkUp Regional Server")
                        model: root.regionOptions
                        textRole: "label"
                        valueRole: "value"
                        Component.onCompleted: {
                            currentIndex = root.regionIndexForCode(settings.country)
                        }
                        onActivated: function(index) {
                            if (index >= 0 && index < root.regionOptions.length) {
                                settings.updateCountry(root.regionOptions[index].value)
                            }
                        }
                        Connections {
                            target: settings
                            function onCountryChanged() {
                                const targetIdx = root.regionIndexForCode(settings.country)
                                if (loginRegionCombo.currentIndex !== targetIdx) {
                                    loginRegionCombo.currentIndex = targetIdx
                                }
                            }
                        }
                    }
                }

                QQC2.CheckBox {
                    objectName: "loginRememberPasswordCheck"
                    visible: settings.dataSource !== "home_assistant"
                    text: qsTr("Remember password in encrypted vault (auto-renew expired tokens)")
                    checked: settings.rememberPassword
                    onToggled: settings.updateRememberPassword(checked)
                    Layout.fillWidth: true
                }

                // Home Assistant LibreLink Integration Fields on Login Card
                QQC2.Label {
                    visible: settings.dataSource === "home_assistant"
                    text: qsTr("Pull live CGM readings, trend arrows, and 12-hour history directly from your Home Assistant LibreLink integration over your local LAN or Tailscale.")
                    wrapMode: Text.WordWrap
                    opacity: 0.85
                    color: Kirigami.Theme.textColor
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource === "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Home Assistant URL")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.TextField {
                        id: loginHaUrlField
                        objectName: "loginHaUrlField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("http://homeassistant.local:8123")
                        text: settings.haUrl
                        Accessible.name: qsTr("Home Assistant URL")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource === "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Long-Lived Access Token")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.TextField {
                        id: loginHaTokenField
                        objectName: "loginHaTokenField"
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: settings.haTokenConfigured
                            ? qsTr("Stored in AES-256-GCM vault (leave blank to keep)")
                            : qsTr("Paste Home Assistant Long-Lived Access Token")
                        Accessible.name: qsTr("Home Assistant Long-Lived Access Token")
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource === "home_assistant" && root.haPatientsModel.length > 0
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Patient / Sensor")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.ComboBox {
                        id: loginHaPatientCombo
                        objectName: "loginHaPatientCombo"
                        Layout.fillWidth: true
                        model: root.haPatientComboOptions
                        textRole: "label"
                        valueRole: "entity_id"
                        currentIndex: root.haPatientComboIndexForEntity(settings.haSensorEntityId)
                        Accessible.name: qsTr("Home Assistant Patient Selection")
                        onActivated: {
                            loginHaEntityField.text = currentValue
                            settingsHaEntityField.text = currentValue
                            telemetry.selectHaPatient(currentValue)
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    visible: settings.dataSource === "home_assistant"
                    spacing: 4
                    QQC2.Label {
                        text: qsTr("Glucose Sensor Entity ID (optional)")
                        font.bold: true
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                    }
                    QQC2.TextField {
                        id: loginHaEntityField
                        objectName: "loginHaEntityField"
                        Layout.fillWidth: true
                        placeholderText: qsTr("Auto-discover (e.g. sensor.patient_glucose_measurement)")
                        text: settings.haSensorEntityId
                        Accessible.name: qsTr("Home Assistant Glucose Sensor Entity ID")
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    QQC2.Button {
                        id: signInButton
                        visible: settings.dataSource !== "home_assistant"
                        Layout.fillWidth: true
                        highlighted: true
                        icon.name: "dialog-ok-apply"
                        text: telemetry.busy ? qsTr("Signing In...") : qsTr("Sign In & Encrypt")
                        enabled: !telemetry.busy
                        onClicked: {
                            const idx = loginRegionCombo.currentIndex >= 0 ? loginRegionCombo.currentIndex : 0
                            const selectedRegion = root.regionOptions[idx].value
                            telemetry.login(
                                loginEmailField.text,
                                loginPasswordField.text,
                                selectedRegion,
                                settings.language
                            )
                            loginPasswordField.text = ""
                        }
                    }

                    QQC2.Button {
                        id: connectHaButton
                        objectName: "connectHaButton"
                        visible: settings.dataSource === "home_assistant"
                        Layout.fillWidth: true
                        highlighted: true
                        icon.name: "network-connect"
                        text: telemetry.busy ? qsTr("Connecting...") : qsTr("Connect Home Assistant")
                        enabled: !telemetry.busy
                        onClicked: {
                            telemetry.saveHomeAssistantConfig(
                                "home_assistant",
                                loginHaUrlField.text,
                                loginHaTokenField.text,
                                loginHaEntityField.text
                            )
                            loginHaTokenField.text = ""
                        }
                    }

                    QQC2.Button {
                        objectName: "loginAutoFillHaButton"
                        visible: settings.dataSource === "home_assistant"
                        icon.name: "document-import"
                        text: qsTr("Auto-Fill Env")
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.text: qsTr("Import HA_URL and HA_TOKEN from ~/.config/home-assistant.env")
                        onClicked: telemetry.autoFillHomeAssistantFromEnv()
                    }

                    QQC2.Button {
                        icon.name: "view-preview"
                        text: qsTr("Sample Data")
                        onClicked: telemetry.loadDemoSampleData()
                    }
                }
            }
        }
    }
}
