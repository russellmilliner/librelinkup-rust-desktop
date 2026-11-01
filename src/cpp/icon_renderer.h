#pragma once

#include <QColor>
#include <QFont>
#include <QIcon>
#include <QLinearGradient>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QPixmap>
#include <QPointF>
#include <QRectF>
#include <QString>
#include <QSvgRenderer>
#include <algorithm>

namespace librelinkup {

inline QPainterPath build_blood_drop_path(qreal sz) {
    const qreal s = sz / 64.0;
    QPainterPath drop;
    drop.moveTo(32.0 * s, 6.5 * s);
    drop.cubicTo(32.0 * s, 6.5 * s, 12.5 * s, 26.5 * s, 12.5 * s, 39.5 * s);
    drop.cubicTo(12.5 * s, 50.0 * s, 21.2 * s, 57.5 * s, 32.0 * s, 57.5 * s);
    drop.cubicTo(42.8 * s, 57.5 * s, 51.5 * s, 50.0 * s, 51.5 * s, 39.5 * s);
    drop.cubicTo(51.5 * s, 26.5 * s, 32.0 * s, 6.5 * s, 32.0 * s, 6.5 * s);
    drop.closeSubpath();
    return drop;
}

inline QPainterPath build_blood_drop_ecg_path(qreal sz) {
    const qreal s = sz / 64.0;
    QPainterPath ecg;
    ecg.moveTo(18.5 * s, 39.5 * s);
    ecg.lineTo(25.5 * s, 39.5 * s);
    ecg.lineTo(29.2 * s, 31.5 * s);
    ecg.lineTo(34.5 * s, 47.0 * s);
    ecg.lineTo(38.0 * s, 39.5 * s);
    ecg.lineTo(45.5 * s, 39.5 * s);
    return ecg;
}

inline QIcon build_multisize_svg_icon(const QString& qrcPath) {
    QSvgRenderer renderer(qrcPath);
    QIcon icon;
    const int sizes[] = {16, 22, 24, 32, 48, 64, 128};
    for (int sz : sizes) {
        QPixmap pm(sz, sz);
        pm.fill(Qt::transparent);
        if (renderer.isValid()) {
            QPainter painter(&pm);
            painter.setRenderHint(QPainter::Antialiasing, true);
            painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
            renderer.render(&painter, QRectF(0, 0, sz, sz));
        }
        icon.addPixmap(pm);
    }
    return icon;
}

inline void draw_outlined_centered_text(
    QPainter& p,
    const QRectF& targetRect,
    const QFont& font,
    const QString& text,
    const QColor& fillColor,
    const QColor& outlineColor,
    qreal outlineWidth
) {
    if (text.isEmpty()) {
        return;
    }
    p.save();
    p.setFont(font);
    const QFontMetricsF fm(font);
    const QRectF tight = fm.tightBoundingRect(text);
    const qreal x = targetRect.center().x() - tight.width() * 0.5 - tight.left();
    const qreal y = targetRect.center().y() + tight.height() * 0.5 - tight.bottom();

    QPainterPath textPath;
    textPath.addText(QPointF(x, y), font, text);
    p.setPen(QPen(outlineColor, outlineWidth, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    p.setBrush(Qt::NoBrush);
    p.drawPath(textPath);
    p.setPen(Qt::NoPen);
    p.setBrush(fillColor);
    p.drawPath(textPath);
    p.restore();
}

inline QIcon build_glucose_badge_icon(
    const QString& badgeText,
    const QString& trendSymbol,
    const QString& colorHex
) {
    QColor bg(colorHex);
    if (!bg.isValid()) {
        bg = QColor(34, 197, 94);
    }
    QIcon icon;
    const int sizes[] = {16, 22, 24, 32, 48, 64, 128};
    for (int sz : sizes) {
        QPixmap pm(sz, sz);
        pm.fill(Qt::transparent);
        QPainter p(&pm);
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setRenderHint(QPainter::TextAntialiasing, true);

        const qreal margin = std::max(0.5, sz * 0.04);
        const QRectF rect(margin, margin, sz - margin * 2.0, sz - margin * 2.0);
        const qreal radius = sz * 0.22;

        // 1. Rounded badge background with range-colored gradient
        QLinearGradient grad(rect.topLeft(), rect.bottomRight());
        grad.setColorAt(0.0, bg.lighter(110));
        grad.setColorAt(1.0, bg.darker(145));
        p.setPen(QPen(QColor(255, 255, 255, 110), std::max(1.0, sz * 0.04)));
        p.setBrush(grad);
        p.drawRoundedRect(rect, radius, radius);

        // 2. Always render the signature blood drop + ECG logo in the background
        const QPainterPath dropPath = build_blood_drop_path(sz);
        p.setPen(QPen(QColor(255, 255, 255, 210), std::max(1.0, sz * 0.032)));
        p.setBrush(QColor(255, 255, 255, 165));
        p.drawPath(dropPath);

        const QPainterPath ecgPath = build_blood_drop_ecg_path(sz);
        p.setPen(QPen(bg.darker(160), std::max(1.0, sz * 0.045), Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        p.setBrush(Qt::NoBrush);
        p.drawPath(ecgPath);

        // 3. Render high-contrast outlined glucose value + trend symbol in the foreground
        QFont f = p.font();
        f.setBold(true);

        const int len = badgeText.length();
        double scale = 0.46;
        if (len >= 4) {
            scale = 0.32;
        } else if (len == 3) {
            scale = 0.38;
        }

        const QColor outlineCol(10, 15, 28, 242);
        const qreal outlineW = std::max(1.8, sz * 0.095);

        if (sz >= 32 && !trendSymbol.isEmpty()) {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale * 0.86)));
            QRectF topRect(rect.left(), rect.top() + sz * 0.16, rect.width(), rect.height() * 0.48);
            draw_outlined_centered_text(p, topRect, f, badgeText, Qt::white, outlineCol, outlineW);

            QFont arrowFont = f;
            arrowFont.setPixelSize(std::max(7, static_cast<int>(sz * 0.23)));
            QRectF botRect(rect.left(), rect.top() + rect.height() * 0.58, rect.width(), rect.height() * 0.36);
            draw_outlined_centered_text(
                p,
                botRect,
                arrowFont,
                trendSymbol,
                Qt::white,
                outlineCol,
                std::max(1.4, sz * 0.075)
            );
        } else {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale)));
            QRectF midRect(rect.left(), rect.top() + sz * 0.08, rect.width(), rect.height() * 0.88);
            draw_outlined_centered_text(p, midRect, f, badgeText, Qt::white, outlineCol, outlineW);
        }
        p.end();
        icon.addPixmap(pm);
    }
    return icon;
}

} // namespace librelinkup
