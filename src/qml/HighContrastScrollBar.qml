import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

QQC2.ScrollBar {
    id: hcScrollBar
    objectName: "highContrastScrollBar"

    property bool isLightTheme: (0.2126 * Kirigami.Theme.backgroundColor.r
        + 0.7152 * Kirigami.Theme.backgroundColor.g
        + 0.0722 * Kirigami.Theme.backgroundColor.b) > 0.45

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
        color: hcScrollBar.isLightTheme
            ? "#cbd5e1"
            : Qt.alpha(Kirigami.Theme.textColor, 0.18)
        border.width: 1
        border.color: hcScrollBar.isLightTheme
            ? "#475569"
            : Qt.alpha(Kirigami.Theme.textColor, 0.36)
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
                : (hcScrollBar.isLightTheme ? "#334155" : Qt.alpha(Kirigami.Theme.textColor, 0.74)))
        border.width: 1
        border.color: hcScrollBar.isLightTheme ? "#0f172a" : Qt.alpha(Kirigami.Theme.backgroundColor, 0.55)
    }
}
