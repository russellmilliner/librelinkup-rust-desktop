#pragma once

#include <QColor>
#include <QFont>
#include <QIcon>
#include <QPainter>
#include <QPainterPath>
#include <QRectF>
#include <QString>

namespace librelinkup {

QPainterPath build_blood_drop_path(qreal sz);
QPainterPath build_blood_drop_ecg_path(qreal sz);
QIcon build_multisize_svg_icon(const QString& qrcPath);
void draw_outlined_centered_text(
    QPainter& p,
    const QRectF& targetRect,
    const QFont& font,
    const QString& text,
    const QColor& fillColor,
    const QColor& outlineColor,
    qreal outlineWidth
);
QIcon build_glucose_badge_icon(
    const QString& badgeText,
    const QString& trendSymbol,
    const QString& colorHex
);

} // namespace librelinkup
