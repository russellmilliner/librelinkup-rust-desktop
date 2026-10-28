#pragma once

#include <QColor>
#include <QFont>
#include <QIcon>
#include <QPainter>
#include <QPen>
#include <QPixmap>
#include <QRectF>
#include <QString>
#include <QSvgRenderer>
#include <algorithm>

namespace librelinkup {

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
        const qreal radius = sz * 0.24;

        p.setPen(QPen(QColor(255, 255, 255, 90), std::max(1.0, sz * 0.04)));
        p.setBrush(bg);
        p.drawRoundedRect(rect, radius, radius);

        p.setPen(Qt::white);
        QFont f = p.font();
        f.setBold(true);

        const int len = badgeText.length();
        double scale = 0.50;
        if (len >= 4) {
            scale = 0.34;
        } else if (len == 3) {
            scale = 0.41;
        }

        if (sz >= 32 && !trendSymbol.isEmpty()) {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale * 0.88)));
            p.setFont(f);
            QRectF topRect(rect.left(), rect.top() + sz * 0.04, rect.width(), rect.height() * 0.62);
            p.drawText(topRect, Qt::AlignCenter, badgeText);

            QFont arrowFont = f;
            arrowFont.setPixelSize(std::max(7, static_cast<int>(sz * 0.26)));
            p.setFont(arrowFont);
            QRectF botRect(rect.left(), rect.top() + rect.height() * 0.54, rect.width(), rect.height() * 0.42);
            p.drawText(botRect, Qt::AlignCenter, trendSymbol);
        } else {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale)));
            p.setFont(f);
            p.drawText(rect, Qt::AlignCenter, badgeText);
        }
        p.end();
        icon.addPixmap(pm);
    }
    return icon;
}

} // namespace librelinkup
