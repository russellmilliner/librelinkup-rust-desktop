import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Rectangle {
    id: graphCard

    required property var root
    required property var telemetry
    required property var settings
    required property var medications

    Connections {
        target: graphCard.telemetry
        function onGraphPointsJsonChanged() {
            glucoseCanvas.updateParsedPoints()
            glucoseCanvas.requestPaint()
        }
        function onResultUnitChanged() {
            glucoseCanvas.requestPaint()
        }
    }

    Connections {
        target: graphCard.settings
        function onThemeModeChanged() { glucoseCanvas.requestPaint() }
        function onHighContrastModeChanged() { glucoseCanvas.requestPaint() }
        function onUiZoomPercentChanged() { glucoseCanvas.requestPaint() }
    }

    Connections {
        target: graphCard.medications
        function onGraphDosesJsonChanged() {
            glucoseCanvas.updateParsedDoses()
            glucoseCanvas.requestPaint()
        }
    }

    Connections {
        target: graphCard.root
        function onZoomScaleChanged() { glucoseCanvas.requestPaint() }
    }

    objectName: "graphCard"
    Layout.fillWidth: true
    Layout.fillHeight: true
    radius: 14
    color: root.cardBgColor
    border.color: root.cardBorderColor
    border.width: root.cardBorderWidth
    Accessible.role: Accessible.Chart
    Accessible.name: qsTr("12-Hour Continuous Glucose Trend Chart")
    Accessible.description: telemetry.accessibleSummary

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            Kirigami.Heading {
                level: 3
                font.pointSize: root.fontHeadingPt
                text: qsTr("Continuous Glucose Trend (%1)").arg(telemetry.resultUnit)
                color: Kirigami.Theme.textColor
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                elide: Text.ElideRight
            }
            QQC2.Label {
                objectName: "graphTargetBandLabel"
                text: glucoseCanvas.hoveredIndex >= 0
                    ? (glucoseCanvas.hoveredValueText + " • " + glucoseCanvas.hoveredTimeText)
                    : qsTr("Target band: %1").arg(telemetry.targetRangeDisplay)
                font.bold: glucoseCanvas.hoveredIndex >= 0
                opacity: glucoseCanvas.hoveredIndex >= 0 ? 0.95 : 0.75
                color: Kirigami.Theme.textColor
                elide: Text.ElideRight
            }
        }

        Canvas {
            id: glucoseCanvas
            objectName: "glucoseCanvas"
            Layout.fillWidth: true
            Layout.fillHeight: true
            activeFocusOnTab: true
            Accessible.role: Accessible.Chart
            Accessible.name: hoveredIndex >= 0
                ? qsTr("CGM Chart Point %1: %2 (%3) at %4")
                    .arg(hoveredIndex + 1)
                    .arg(hoveredValueText)
                    .arg(hoveredStatusText)
                    .arg(hoveredTimeText)
                : qsTr("12-Hour Continuous Glucose Trend Chart (%1). Use Left and Right arrow keys to inspect individual readings.")
                    .arg(telemetry.targetRangeDisplay)
            Accessible.description: telemetry.accessibleSummary

            property int hoveredIndex: -1
            property real hoveredX: 0
            property real hoveredY: 0
            property string hoveredValueText: ""
            property string hoveredStatusText: ""
            property string hoveredTimeText: ""
            property string hoveredColorHex: "#22c55e"
            property var parsedPoints: []
            property var parsedDoseMarkers: []

            function updateParsedPoints() {
                try {
                    parsedPoints = JSON.parse(telemetry.graphPointsJson || "[]")
                } catch (e) {
                    parsedPoints = []
                }
            }

            function updateParsedDoses() {
                try {
                    parsedDoseMarkers = JSON.parse(medications.graphDosesJson || "[]")
                } catch (e) {
                    parsedDoseMarkers = []
                }
            }

            Component.onCompleted: {
                updateParsedPoints()
                updateParsedDoses()
            }

            function clearHover() {
                if (hoveredIndex !== -1) {
                    hoveredIndex = -1
                    hoveredValueText = ""
                    hoveredStatusText = ""
                    hoveredTimeText = ""
                    requestPaint()
                }
            }

            function selectPointByIndex(idx) {
                const pts = parsedPoints
                if (pts.length === 0 || width <= 0 || height <= 0) {
                    clearHover()
                    return
                }
                const clampedIdx = Math.max(0, Math.min(pts.length - 1, idx))
                const padLeft = 48
                const padRight = 20
                const padTop = 16
                const padBottom = 28
                const plotW = Math.max(10, width - padLeft - padRight)
                const plotH = Math.max(10, height - padTop - padBottom)

                let minMg = 50
                let maxMg = 260
                for (let i = 0; i < pts.length; ++i) {
                    const v = Number(pts[i].value_mg_dl || 0)
                    if (v > 0) {
                        if (v - 20 < minMg) minMg = Math.max(30, v - 20)
                        if (v + 20 > maxMg) maxMg = Math.min(420, v + 20)
                    }
                }
                const yForMg = function(mg) {
                    const ratio = (mg - minMg) / Math.max(1, (maxMg - minMg))
                    return padTop + plotH * (1.0 - Math.max(0.0, Math.min(1.0, ratio)))
                }

                const pt = pts[clampedIdx]
                const mg = Number(pt.value_mg_dl || 0)
                const px = pts.length === 1
                    ? (padLeft + plotW / 2)
                    : (padLeft + (plotW * clampedIdx) / (pts.length - 1))
                const py = yForMg(mg)
                const isMmol = telemetry.resultUnit === "mmol/L"
                const formattedVal = isMmol
                    ? (mg / 18.0).toFixed(1) + " mmol/L"
                    : Math.round(mg).toString() + " mg/dL"

                let statusLabel = qsTr("In Range")
                let ptColor = "#22c55e"
                if (mg < 70) {
                    statusLabel = qsTr("Low")
                    ptColor = "#ef4444"
                } else if (mg > 240) {
                    statusLabel = qsTr("High")
                    ptColor = "#f97316"
                } else if (mg < telemetry.targetLowMgDl || mg > telemetry.targetHighMgDl) {
                    statusLabel = qsTr("Out of Target")
                    ptColor = "#eab308"
                }

                hoveredIndex = clampedIdx
                hoveredX = px
                hoveredY = py
                hoveredValueText = formattedVal
                hoveredStatusText = statusLabel
                let timeStr = String(pt.timestamp || qsTr("Unknown time"))
                const dMarkers = parsedDoseMarkers
                for (let d = 0; d < dMarkers.length; ++d) {
                    const dm = dMarkers[d]
                    if (Math.abs(Number(dm.point_index || 0) - clampedIdx) <= 0.75) {
                        timeStr += " • 💊 " + String(dm.medication_name || "") + " (" + String(dm.dosage || "") + ")"
                    }
                }
                hoveredTimeText = timeStr
                hoveredColorHex = ptColor
                requestPaint()
            }

            onActiveFocusChanged: requestPaint()

            Keys.onPressed: function(event) {
                const pts = parsedPoints
                if (event.key === Qt.Key_Left && pts.length > 0) {
                    const nextIdx = hoveredIndex < 0 ? (pts.length - 1) : Math.max(0, hoveredIndex - 1)
                    selectPointByIndex(nextIdx)
                    event.accepted = true
                } else if (event.key === Qt.Key_Right && pts.length > 0) {
                    const nextIdx = hoveredIndex < 0 ? (pts.length - 1) : Math.min(pts.length - 1, hoveredIndex + 1)
                    selectPointByIndex(nextIdx)
                    event.accepted = true
                } else if (event.key === Qt.Key_Home && pts.length > 0) {
                    selectPointByIndex(0)
                    event.accepted = true
                } else if (event.key === Qt.Key_End && pts.length > 0) {
                    selectPointByIndex(pts.length - 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Escape) {
                    clearHover()
                    event.accepted = true
                }
            }

            function updateHoverAt(mx, my) {
                const pts = parsedPoints
                if (pts.length === 0 || width <= 0 || height <= 0) {
                    clearHover()
                    return
                }

                const padLeft = 48
                const padRight = 20
                const padTop = 16
                const padBottom = 28
                const plotW = Math.max(10, width - padLeft - padRight)
                const plotH = Math.max(10, height - padTop - padBottom)

                if (mx < padLeft - 16 || mx > width - padRight + 16 || my < 0 || my > height) {
                    clearHover()
                    return
                }

                let minMg = 50
                let maxMg = 260
                for (let i = 0; i < pts.length; ++i) {
                    const v = Number(pts[i].value_mg_dl || 0)
                    if (v > 0) {
                        if (v - 20 < minMg) minMg = Math.max(30, v - 20)
                        if (v + 20 > maxMg) maxMg = Math.min(420, v + 20)
                    }
                }

                const yForMg = function(mg) {
                    const ratio = (mg - minMg) / Math.max(1, (maxMg - minMg))
                    return padTop + plotH * (1.0 - Math.max(0.0, Math.min(1.0, ratio)))
                }

                // Find nearest point horizontally (with 2D distance tie-break)
                let bestIdx = -1
                let bestDist = 1e9
                let bestX = 0
                let bestY = 0
                for (let i = 0; i < pts.length; ++i) {
                    const v = Number(pts[i].value_mg_dl || 0)
                    const px = pts.length === 1
                        ? (padLeft + plotW / 2)
                        : (padLeft + (plotW * i) / (pts.length - 1))
                    const py = yForMg(v)
                    const dx = Math.abs(mx - px)
                    const dy = Math.abs(my - py)
                    const score = dx + dy * 0.15
                    if (score < bestDist) {
                        bestDist = score
                        bestIdx = i
                        bestX = px
                        bestY = py
                    }
                }

                if (bestIdx < 0) {
                    clearHover()
                    return
                }

                const pt = pts[bestIdx]
                const mg = Number(pt.value_mg_dl || 0)
                const isMmol = telemetry.resultUnit === "mmol/L"
                const formattedVal = isMmol
                    ? (mg / 18.0).toFixed(1) + " mmol/L"
                    : Math.round(mg).toString() + " mg/dL"

                let statusLabel = qsTr("In Range")
                let ptColor = "#22c55e"
                if (mg < 70) {
                    statusLabel = qsTr("Low")
                    ptColor = "#ef4444"
                } else if (mg > 240) {
                    statusLabel = qsTr("High")
                    ptColor = "#f97316"
                } else if (mg < telemetry.targetLowMgDl || mg > telemetry.targetHighMgDl) {
                    statusLabel = qsTr("Out of Target")
                    ptColor = "#eab308"
                }

                hoveredIndex = bestIdx
                hoveredX = bestX
                hoveredY = bestY
                hoveredValueText = formattedVal
                hoveredStatusText = statusLabel
                let timeStr = String(pt.timestamp || qsTr("Unknown time"))
                const dMarkers = parsedDoseMarkers
                for (let d = 0; d < dMarkers.length; ++d) {
                    const dm = dMarkers[d]
                    if (Math.abs(Number(dm.point_index || 0) - bestIdx) <= 0.75) {
                        timeStr += " • 💊 " + String(dm.medication_name || "") + " (" + String(dm.dosage || "") + ")"
                    }
                }
                hoveredTimeText = timeStr
                hoveredColorHex = ptColor
                requestPaint()
            }

            onWidthChanged: {
                clearHover()
                requestPaint()
            }
            onHeightChanged: {
                clearHover()
                requestPaint()
            }

            HoverHandler {
                id: canvasHoverHandler
                onPointChanged: {
                    if (hovered) {
                        glucoseCanvas.updateHoverAt(point.position.x, point.position.y)
                    }
                }
                onHoveredChanged: {
                    if (!hovered) {
                        glucoseCanvas.clearHover()
                    }
                }
            }

            MouseArea {
                id: canvasHoverMouseArea
                objectName: "glucoseCanvasMouseArea"
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton
                onPositionChanged: function(mouse) {
                    glucoseCanvas.updateHoverAt(mouse.x, mouse.y)
                }
                onPressed: function(mouse) {
                    glucoseCanvas.updateHoverAt(mouse.x, mouse.y)
                }
                onExited: {
                    if (!canvasHoverHandler.hovered) {
                        glucoseCanvas.clearHover()
                    }
                }
            }

            Rectangle {
                id: graphHoverTooltip
                objectName: "graphHoverTooltip"
                visible: glucoseCanvas.hoveredIndex >= 0
                z: 10
                radius: 8
                color: root.isLightTheme
                    ? Qt.rgba(0.98, 0.99, 1.0, 0.96)
                    : Qt.rgba(0.09, 0.12, 0.18, 0.95)
                border.color: glucoseCanvas.hoveredColorHex
                border.width: 1.5
                width: Math.min(
                    Math.max(120, Math.min(240, tooltipCol.implicitWidth + 20)),
                    Math.max(120, glucoseCanvas.width - 16)
                )
                height: tooltipCol.implicitHeight + 14
                implicitWidth: width
                implicitHeight: height

                x: {
                    const preferredRight = glucoseCanvas.hoveredX + 12
                    const preferredLeft = glucoseCanvas.hoveredX - width - 12
                    const rawX = (preferredRight + width + 6 <= glucoseCanvas.width)
                        ? preferredRight
                        : preferredLeft
                    return Math.max(6, Math.min(glucoseCanvas.width - width - 6, rawX))
                }
                y: {
                    const preferredAbove = glucoseCanvas.hoveredY - height - 10
                    const preferredBelow = glucoseCanvas.hoveredY + 12
                    const rawY = (preferredAbove >= 6) ? preferredAbove : preferredBelow
                    return Math.max(6, Math.min(glucoseCanvas.height - height - 6, rawY))
                }

                ColumnLayout {
                    id: tooltipCol
                    objectName: "graphHoverTooltipCol"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 7
                    spacing: 2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Rectangle {
                            Layout.preferredWidth: 10
                            Layout.preferredHeight: 10
                            radius: 5
                            color: glucoseCanvas.hoveredColorHex
                        }

                        QQC2.Label {
                            objectName: "graphHoverValueLabel"
                            text: glucoseCanvas.hoveredValueText
                            font.bold: true
                            font.pointSize: root.fontSubheadingPt
                            color: Kirigami.Theme.textColor
                        }

                        QQC2.Label {
                            objectName: "graphHoverStatusLabel"
                            text: "(" + glucoseCanvas.hoveredStatusText + ")"
                            font.pointSize: root.fontSmallPt
                            font.bold: true
                            color: glucoseCanvas.hoveredColorHex
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }

                    QQC2.Label {
                        objectName: "graphHoverTimeLabel"
                        text: glucoseCanvas.hoveredTimeText
                        font.pointSize: root.fontSmallPt
                        opacity: 0.85
                        color: Kirigami.Theme.textColor
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                }
            }

            onPaint: {
                const ctx = getContext("2d")
                ctx.clearRect(0, 0, width, height)

                const pts = parsedPoints

                const padLeft = Math.round(48 * Math.min(1.25, root.zoomScale))
                const padRight = 20
                const padTop = 16
                const padBottom = 28
                const plotW = Math.max(10, width - padLeft - padRight)
                const plotH = Math.max(10, height - padTop - padBottom)

                const isMmol = telemetry.resultUnit === "mmol/L"
                const conv = function(mg) {
                    return isMmol ? (mg / 18.0) : mg
                }

                let minMg = 50
                let maxMg = 260
                for (let i = 0; i < pts.length; ++i) {
                    const v = Number(pts[i].value_mg_dl || 0)
                    if (v > 0) {
                        if (v - 20 < minMg) minMg = Math.max(30, v - 20)
                        if (v + 20 > maxMg) maxMg = Math.min(420, v + 20)
                    }
                }

                const yForMg = function(mg) {
                    const ratio = (mg - minMg) / Math.max(1, (maxMg - minMg))
                    return padTop + plotH * (1.0 - Math.max(0.0, Math.min(1.0, ratio)))
                }

                // Plot area subtle background & frame in Light theme for clear contrast
                if (root.isLightTheme) {
                    ctx.fillStyle = "#f8fafc"
                    ctx.fillRect(padLeft, padTop, plotW, plotH)
                }
                if (glucoseCanvas.activeFocus) {
                    ctx.save()
                    ctx.strokeStyle = root.isLightTheme ? "#0284c7" : "#38bdf8"
                    ctx.lineWidth = 2
                    ctx.strokeRect(padLeft - 2, padTop - 2, plotW + 4, plotH + 4)
                    ctx.restore()
                }

                // Target range shaded band
                const yHigh = yForMg(telemetry.targetHighMgDl)
                const yLow = yForMg(telemetry.targetLowMgDl)
                ctx.fillStyle = root.isLightTheme
                    ? "rgba(22, 163, 74, 0.18)"
                    : "rgba(34, 197, 94, 0.18)"
                ctx.fillRect(padLeft, yHigh, plotW, Math.max(2, yLow - yHigh))

                // Grid lines
                ctx.strokeStyle = root.isLightTheme
                    ? "rgba(30, 41, 59, 0.26)"
                    : "rgba(148, 163, 184, 0.18)"
                ctx.lineWidth = 1
                ctx.font = "bold " + Math.round(11 * root.zoomScale) + "px sans-serif"
                ctx.fillStyle = root.isLightTheme ? "#0f172a" : "#cbd5e1"

                const steps = 4
                for (let s = 0; s <= steps; ++s) {
                    const mgVal = minMg + ((maxMg - minMg) * s) / steps
                    const y = yForMg(mgVal)
                    ctx.beginPath()
                    ctx.moveTo(padLeft, y)
                    ctx.lineTo(padLeft + plotW, y)
                    ctx.stroke()

                    const labelText = isMmol
                        ? conv(mgVal).toFixed(1)
                        : Math.round(mgVal).toString()
                    ctx.fillText(labelText, 8, y + 4)
                }

                if (pts.length === 0) {
                    ctx.font = Math.round(13 * root.zoomScale) + "px sans-serif"
                    ctx.fillText(
                        "No CGM graph points received yet — click Refresh or Sample Data.",
                        padLeft + 16,
                        padTop + plotH / 2
                    )
                    return
                }

                // Plot line
                ctx.strokeStyle = root.isLightTheme ? "#0284c7" : "#38bdf8"
                ctx.lineWidth = 2.5
                ctx.beginPath()
                for (let i = 0; i < pts.length; ++i) {
                    const x = pts.length === 1
                        ? (padLeft + plotW / 2)
                        : (padLeft + (plotW * i) / (pts.length - 1))
                    const y = yForMg(Number(pts[i].value_mg_dl || 0))
                    if (i === 0) ctx.moveTo(x, y)
                    else ctx.lineTo(x, y)
                }
                ctx.stroke()

                // Vertical crosshair guide when hovering a point
                if (hoveredIndex >= 0 && hoveredIndex < pts.length) {
                    ctx.save()
                    ctx.strokeStyle = root.isLightTheme
                        ? "rgba(15, 23, 42, 0.35)"
                        : "rgba(248, 250, 252, 0.35)"
                    ctx.lineWidth = 1
                    ctx.beginPath()
                    ctx.moveTo(hoveredX, padTop)
                    ctx.lineTo(hoveredX, padTop + plotH)
                    ctx.stroke()
                    ctx.restore()
                }

                // Plot points
                for (let i = 0; i < pts.length; ++i) {
                    const v = Number(pts[i].value_mg_dl || 0)
                    const x = pts.length === 1
                        ? (padLeft + plotW / 2)
                        : (padLeft + (plotW * i) / (pts.length - 1))
                    const y = yForMg(v)

                    let ptColor = "#22c55e"
                    if (v < 70) ptColor = "#ef4444"
                    else if (v > 240) ptColor = "#f97316"
                    else if (v < telemetry.targetLowMgDl || v > telemetry.targetHighMgDl) ptColor = "#eab308"

                    if (i === hoveredIndex) {
                        ctx.fillStyle = root.isLightTheme
                            ? "rgba(2, 132, 199, 0.22)"
                            : "rgba(56, 189, 248, 0.28)"
                        ctx.beginPath()
                        ctx.arc(x, y, 9, 0, Math.PI * 2)
                        ctx.fill()

                        ctx.fillStyle = ptColor
                        ctx.beginPath()
                        ctx.arc(x, y, 5.5, 0, Math.PI * 2)
                        ctx.fill()

                        ctx.strokeStyle = "#ffffff"
                        ctx.lineWidth = 2
                        ctx.stroke()
                    } else {
                        ctx.fillStyle = ptColor
                        ctx.beginPath()
                        ctx.arc(x, y, 3.5, 0, Math.PI * 2)
                        ctx.fill()
                    }
                }

                // Logged Medication Dose Markers (#a855f7 diamond + vertical guide)
                const doseMarkers = parsedDoseMarkers
                for (let d = 0; d < doseMarkers.length; ++d) {
                    const dm = doseMarkers[d]
                    const pIdx = Number(dm.point_index || 0)
                    const mx = pts.length <= 1
                        ? (padLeft + plotW / 2)
                        : (padLeft + (plotW * pIdx) / (pts.length - 1))
                    const my = yForMg(Number(dm.value_mg_dl || 110))

                    ctx.save()
                    ctx.strokeStyle = "rgba(168, 85, 247, 0.55)"
                    ctx.lineWidth = 1.5
                    ctx.beginPath()
                    ctx.moveTo(mx, padTop)
                    ctx.lineTo(mx, padTop + plotH)
                    ctx.stroke()

                    // Purple diamond marker
                    ctx.fillStyle = "#a855f7"
                    ctx.beginPath()
                    ctx.moveTo(mx, my - 7)
                    ctx.lineTo(mx + 7, my)
                    ctx.lineTo(mx, my + 7)
                    ctx.lineTo(mx - 7, my)
                    ctx.closePath()
                    ctx.fill()
                    ctx.strokeStyle = "#ffffff"
                    ctx.lineWidth = 1.5
                    ctx.stroke()

                    // Compact label above marker
                    const medTag = String(dm.medication_name || "Dose")
                    ctx.font = "bold " + Math.round(10 * root.zoomScale) + "px sans-serif"
                    ctx.fillStyle = root.isLightTheme ? "#6b21a8" : "#e9d5ff"
                    const labelX = Math.max(padLeft + 4, Math.min(padLeft + plotW - 72, mx - 18))
                    const labelY = Math.max(padTop + 12, my - 11)
                    ctx.fillText("💊 " + medTag, labelX, labelY)
                    ctx.restore()
                }
            }
        }
    }
}
