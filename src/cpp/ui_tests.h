#pragma once

#include <QApplication>
#include <QColor>
#include <QCoreApplication>
#include <QDir>
#include <QEventLoop>
#include <QFont>
#include <QIcon>
#include <QImage>
#include <QLinearGradient>
#include <QList>
#include <QLoggingCategory>
#include <QMetaObject>
#include <QObject>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QPixmap>
#include <QPoint>
#include <QPointF>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRect>
#include <QRectF>
#include <QScreen>
#include <QSize>
#include <QString>
#include <QStringList>
#include <QThread>
#include <QUrl>
#include <QVariant>
#include <QVector>
#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <memory>

#include "icon_renderer.h"
#include "tray_manager.h"
#include "window_manager.h"

namespace librelinkup_uitest {

inline bool is_visible_in_hierarchy(QQuickItem* item) {
    for (QQuickItem* cur = item; cur != nullptr; cur = cur->parentItem()) {
        if (!cur->isVisible() || cur->opacity() <= 0.01) {
            return false;
        }
    }
    return true;
}

inline void inspect_item_tree(
    QQuickItem* item,
    QQuickWindow* win,
    const QString& contextLabel,
    QStringList& violations
) {
    if (!item || !is_visible_in_hierarchy(item)) {
        return;
    }

    const QString objName = item->objectName();
    const QString className = QString::fromLatin1(item->metaObject()->className());
    const QString label = objName.isEmpty() ? className : QStringLiteral("%1(%2)").arg(objName, className);

    // 1. Check horizontal & vertical Layout implicit size overflows
    if (className.contains(QStringLiteral("QQuickRowLayout")) ||
        className.contains(QStringLiteral("QQuickColumnLayout")) ||
        className.contains(QStringLiteral("QQuickGridLayout"))) {
        if (item->width() > 1.0 && item->implicitWidth() > item->width() + 2.0) {
            violations.append(
                QStringLiteral("[%1] Layout '%2' implicitWidth (%3) overflows allocated width (%4)")
                    .arg(contextLabel, label)
                    .arg(item->implicitWidth(), 0, 'f', 1)
                    .arg(item->width(), 0, 'f', 1)
            );
        }
        if (item->height() > 1.0 && item->implicitHeight() > item->height() + 2.0) {
            violations.append(
                QStringLiteral("[%1] Layout '%2' implicitHeight (%3) overflows allocated height (%4)")
                    .arg(contextLabel, label)
                    .arg(item->implicitHeight(), 0, 'f', 1)
                    .arg(item->height(), 0, 'f', 1)
            );
        }
    }

    // 2. Check named card containers to ensure their inner content column fits inside the card height & width
    static const QStringList cardNames = {
        QStringLiteral("loginCard"),
        QStringLiteral("heroCard"),
        QStringLiteral("detailsCard"),
        QStringLiteral("graphCard"),
        QStringLiteral("ea1cAnalyticsCard"),
        QStringLiteral("medicationsScheduleCard"),
        QStringLiteral("addMedicationCard"),
        QStringLiteral("doseHistoryCard"),
        QStringLiteral("traySettingsCard"),
        QStringLiteral("displaySettingsCard"),
        QStringLiteral("alertsSettingsCard"),
        QStringLiteral("accountSecurityCard"),
        QStringLiteral("logListCard"),
        QStringLiteral("aboutHeaderCard"),
        QStringLiteral("aboutLinksCard"),
        QStringLiteral("overlayPill"),
        QStringLiteral("mainHeaderToolBar")
    };
    if (cardNames.contains(objName)) {
        const qreal cardW = item->width();
        const qreal cardH = item->height();
        for (QQuickItem* child : item->childItems()) {
            if (!child || !child->isVisible() || child->width() <= 0.0 || child->height() <= 0.0) {
                continue;
            }
            const QString childClass = QString::fromLatin1(child->metaObject()->className());
            if (childClass.contains(QStringLiteral("MouseArea"))) {
                continue;
            }
            const qreal rightEdge = child->x() + child->width();
            const qreal bottomEdge = child->y() + child->height();
            const qreal childImpW = child->implicitWidth();
            const qreal childImpH = child->implicitHeight();

            if (rightEdge > cardW + 2.0 || childImpW > cardW + 2.0) {
                violations.append(
                    QStringLiteral("[%1] Child '%2' (right=%3, impW=%4) overflows card '%5' width (%6)")
                        .arg(contextLabel, child->objectName().isEmpty() ? childClass : child->objectName())
                        .arg(rightEdge, 0, 'f', 1)
                        .arg(childImpW, 0, 'f', 1)
                        .arg(objName)
                        .arg(cardW, 0, 'f', 1)
                );
            }
            if (bottomEdge > cardH + 2.0 || childImpH > cardH + 2.0) {
                violations.append(
                    QStringLiteral("[%1] Child '%2' (bottom=%3, impH=%4) overflows card '%5' height (%6)")
                        .arg(contextLabel, child->objectName().isEmpty() ? childClass : child->objectName())
                        .arg(bottomEdge, 0, 'f', 1)
                        .arg(childImpH, 0, 'f', 1)
                        .arg(objName)
                        .arg(cardH, 0, 'f', 1)
                );
            }
        }
    }

    // 3. Check that visible controls/cards do not overflow the window's horizontal bounds
    if (!objName.isEmpty() && item->width() > 1.0 && win) {
        const QPointF topLeft = item->mapToScene(QPointF(0, 0));
        const qreal rightInWin = topLeft.x() + item->width();
        if (topLeft.x() < -2.0 || rightInWin > win->width() + 2.0) {
            const QString parentInfo = item->parentItem()
                ? QStringLiteral("%1(%2)")
                      .arg(item->parentItem()->objectName(), QString::fromLatin1(item->parentItem()->metaObject()->className()))
                : QStringLiteral("none");
            const QString txtProp = item->property("text").toString();
            violations.append(
                QStringLiteral("[%1] Item '%2' parent='%3' text='%4' [x=%5..%6] overflows window width (%7)")
                    .arg(contextLabel, label, parentInfo, txtProp)
                    .arg(topLeft.x(), 0, 'f', 1)
                    .arg(rightInWin, 0, 'f', 1)
                    .arg(win->width())
            );
        }
    }

    for (QQuickItem* child : item->childItems()) {
        inspect_item_tree(child, win, contextLabel, violations);
    }
}

inline void inspect_window_tree(
    QQuickWindow* win,
    const QString& contextLabel,
    QStringList& violations
) {
    if (!win) {
        return;
    }
    if (QQuickItem* headerItem = qobject_cast<QQuickItem*>(win->property("header").value<QObject*>())) {
        inspect_item_tree(headerItem, win, contextLabel, violations);
    }
    inspect_item_tree(win->contentItem(), win, contextLabel, violations);
}

inline void pump_events(int ms = 40) {
    QCoreApplication::processEvents(QEventLoop::AllEvents, ms);
    QThread::msleep(10);
    QCoreApplication::processEvents(QEventLoop::AllEvents, ms);
}

} // namespace librelinkup_uitest

inline QString librelinkup_run_ui_overflow_tests() {
    qputenv("QT_QPA_PLATFORM", "offscreen");
    QLoggingCategory::setFilterRules(QStringLiteral("union.*=false\nqt.core.qobject.connect=false"));
    if (!std::getenv("QT_QUICK_CONTROLS_STYLE")) {
        qputenv("QT_QUICK_CONTROLS_STYLE", "org.kde.breeze");
    }

    static int argc = 1;
    static char arg0[] = "librelinkup-ui-test";
    static char* argv[] = {arg0, nullptr};

    QApplication* app = qobject_cast<QApplication*>(QCoreApplication::instance());
    std::unique_ptr<QApplication> ownedApp;
    if (!app) {
        ownedApp.reset(new QApplication(argc, argv));
        app = ownedApp.get();
    }
    librelinkup::install_wheel_scroll_filter();

    // Surface the actual QML diagnostics: a silent "failed to load" is undebuggable in CI.
    QQmlApplicationEngine engine;
    QStringList qmlErrors;
    QObject::connect(&engine, &QQmlApplicationEngine::warnings, &engine,
        [&qmlErrors](const QList<QQmlError>& warnings) {
            for (const QQmlError& e : warnings) {
                qmlErrors.append(e.toString());
                qWarning("librelinkup-uitest: %s", qPrintable(e.toString()));
            }
        });
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/io/github/librelinkup/src/qml/Main.qml")));
    if (engine.rootObjects().isEmpty()) {
        return QStringLiteral("FAIL: Failed to load Main.qml in QQmlApplicationEngine\n")
            + qmlErrors.join(QLatin1Char('\n'));
    }

    QQuickWindow* mainWin = qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    if (!mainWin) {
        return QStringLiteral("FAIL: Root object is not a QQuickWindow");
    }

    QObject* ctrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupAppController"));
    if (!ctrl) {
        return QStringLiteral("FAIL: Could not find librelinkupAppController (TelemetryController)");
    }
    QObject* settingsCtrl =
        mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"));
    if (!settingsCtrl) {
        return QStringLiteral("FAIL: Could not find librelinkupSettingsController");
    }
    QObject* medsCtrl =
        mainWin->findChild<QObject*>(QStringLiteral("librelinkupMedicationsController"));
    if (!medsCtrl) {
        return QStringLiteral("FAIL: Could not find librelinkupMedicationsController");
    }
    // The QML UI is pull-based: a 250 ms timer runs `telemetry.tick()` and `sync()` on the
    // other two controllers. The headless harness must drive that explicitly instead of
    // sleeping long enough for the timer to fire.
    auto sync_all = [ctrl, settingsCtrl, medsCtrl]() {
        QMetaObject::invokeMethod(ctrl, "tick", Qt::DirectConnection);
        QMetaObject::invokeMethod(settingsCtrl, "sync", Qt::DirectConnection);
        QMetaObject::invokeMethod(medsCtrl, "sync", Qt::DirectConnection);
    };

    QStringList violations;
    const QList<QSize> testSizes = {
        QSize(780, 540),  // Minimum window size
        QSize(980, 680),  // Default window size
        QSize(1280, 800)  // Large desktop size
    };
    const QStringList tabs = {
        QStringLiteral("dashboard"),
        QStringLiteral("medications"),
        QStringLiteral("general"),
        QStringLiteral("alerts"),
        QStringLiteral("account"),
        QStringLiteral("logs"),
        QStringLiteral("about")
    };

    mainWin->show();
    librelinkup_uitest::pump_events(60);

    // State A: Logged out (Login form on dashboard) + all tabs across all sizes
    QMetaObject::invokeMethod(ctrl, "logout", Qt::DirectConnection);
    librelinkup_uitest::pump_events(40);

    for (const QSize& sz : testSizes) {
        mainWin->resize(sz);
        librelinkup_uitest::pump_events(50);
        for (const QString& tab : tabs) {
            mainWin->setProperty("activeTab", tab);
            librelinkup_uitest::pump_events(40);
            const QString ctx = QStringLiteral("LoggedOut %1x%2 tab=%3")
                .arg(sz.width())
                .arg(sz.height())
                .arg(tab);
            librelinkup_uitest::inspect_window_tree(mainWin, ctx, violations);
        }
    }

    // State B: Logged in (Demo sample data loaded -> Hero Card, Details Card, 12-Hour Graph)
    // across both Dark and Light themes and all sizes/tabs
    QMetaObject::invokeMethod(ctrl, "loadDemoSampleData", Qt::DirectConnection);
    sync_all();
    librelinkup_uitest::pump_events(50);

    for (const QString& theme : {QStringLiteral("dark"), QStringLiteral("light")}) {
        QMetaObject::invokeMethod(
            ctrl,
            "updateThemeMode",
            Qt::DirectConnection,
            Q_ARG(QString, theme)
        );
        sync_all();
        librelinkup_uitest::pump_events(40);

        for (const QSize& sz : testSizes) {
            mainWin->resize(sz);
            librelinkup_uitest::pump_events(50);
            for (const QString& tab : tabs) {
                mainWin->setProperty("activeTab", tab);
                librelinkup_uitest::pump_events(40);
                const QString ctx = QStringLiteral("LoggedIn(%1) %2x%3 tab=%4")
                    .arg(theme)
                    .arg(sz.width())
                    .arg(sz.height())
                    .arg(tab);
                librelinkup_uitest::inspect_window_tree(mainWin, ctx, violations);

                if (tab == QStringLiteral("medications")) {
                    if (QQuickItem* schedCombo = mainWin->findChild<QQuickItem*>(QStringLiteral("medScheduleCombo"))) {
                        schedCombo->setProperty("currentIndex", 2); // Weekly -> shows medWeekdayCombo
                        mainWin->setProperty("editingMedId", QStringLiteral("demo-med-1"));
                        librelinkup_uitest::pump_events(25);
                        librelinkup_uitest::inspect_item_tree(
                            mainWin->contentItem(),
                            mainWin,
                            ctx + QStringLiteral(" weeklySchedule+editing"),
                            violations
                        );
                        mainWin->setProperty("editingMedId", QStringLiteral(""));
                        schedCombo->setProperty("currentIndex", 0);
                    }
                }

                if (tab == QStringLiteral("dashboard")) {
                    if (QQuickItem* canvas = mainWin->findChild<QQuickItem*>(QStringLiteral("glucoseCanvas"))) {
                        const qreal cw = canvas->width();
                        const qreal ch = canvas->height();
                        for (const qreal rx : {qreal(52.0), cw * 0.5, cw - 24.0}) {
                            QMetaObject::invokeMethod(
                                canvas,
                                "updateHoverAt",
                                Qt::DirectConnection,
                                Q_ARG(QVariant, QVariant(rx)),
                                Q_ARG(QVariant, QVariant(ch * 0.5))
                            );
                            librelinkup_uitest::pump_events(25);
                            const int hIdx = canvas->property("hoveredIndex").toInt();
                            const QString hVal = canvas->property("hoveredValueText").toString();
                            const QString hTime = canvas->property("hoveredTimeText").toString();
                            if (hIdx < 0 || hVal.isEmpty() || hTime.isEmpty()) {
                                violations.append(
                                    QStringLiteral("[%1] Graph hover at x=%2 failed to resolve point data (idx=%3, val='%4', time='%5')")
                                        .arg(ctx)
                                        .arg(rx, 0, 'f', 1)
                                        .arg(hIdx)
                                        .arg(hVal, hTime)
                                );
                            }
                            if (QQuickItem* tip = mainWin->findChild<QQuickItem*>(QStringLiteral("graphHoverTooltip"))) {
                                if (!tip->isVisible()) {
                                    violations.append(QStringLiteral("[%1] graphHoverTooltip is not visible when hovering point %2").arg(ctx).arg(hIdx));
                                } else if (tip->x() < 0.0 || tip->y() < 0.0 ||
                                           tip->x() + tip->width() > cw + 2.0 ||
                                           tip->y() + tip->height() > ch + 2.0) {
                                    violations.append(
                                        QStringLiteral("[%1] graphHoverTooltip (%2,%3 %4x%5) overflows glucoseCanvas (%6x%7)")
                                            .arg(ctx)
                                            .arg(tip->x(), 0, 'f', 1)
                                            .arg(tip->y(), 0, 'f', 1)
                                            .arg(tip->width(), 0, 'f', 1)
                                            .arg(tip->height(), 0, 'f', 1)
                                            .arg(cw, 0, 'f', 1)
                                            .arg(ch, 0, 'f', 1)
                                    );
                                }
                            }
                            librelinkup_uitest::inspect_item_tree(
                                mainWin->contentItem(),
                                mainWin,
                                ctx + QStringLiteral(" hoverX=%1").arg(rx, 0, 'f', 0),
                                violations
                            );
                        }
                        QMetaObject::invokeMethod(canvas, "clearHover", Qt::DirectConnection);
                    }
                }
            }
        }
    }

    // State C: Verify openLogViewer() and openAboutDialog() switch inline activeTab
    // and never spawn separate top-level windows
    QMetaObject::invokeMethod(mainWin, "openLogViewer", Qt::DirectConnection);
    librelinkup_uitest::pump_events(30);
    if (mainWin->property("activeTab").toString() != QStringLiteral("logs")) {
        violations.append(QStringLiteral("openLogViewer() did not switch activeTab to 'logs'"));
    }
    if (librelinkup::find_log_viewer_window() != nullptr) {
        violations.append(QStringLiteral("Logs view should be an inline tab, not a separate top-level window"));
    }

    QMetaObject::invokeMethod(mainWin, "openAboutDialog", Qt::DirectConnection);
    librelinkup_uitest::pump_events(30);
    if (mainWin->property("activeTab").toString() != QStringLiteral("about")) {
        violations.append(QStringLiteral("openAboutDialog() did not switch activeTab to 'about'"));
    }
    if (librelinkup::find_about_window() != nullptr) {
        violations.append(QStringLiteral("About view should be an inline tab, not a separate top-level window"));
    }
    mainWin->setProperty("activeTab", QStringLiteral("dashboard"));
    librelinkup_uitest::pump_events(20);

    // Verify interactive desktop reminder notification actions ("med_taken:<id>" and "med_skip:<id>")
    bool takenOk = false;
    QMetaObject::invokeMethod(
        ctrl,
        "handleNotificationAction",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, takenOk),
        Q_ARG(QString, QStringLiteral("med_taken:demo-med-1"))
    );
    if (!takenOk) {
        violations.append(QStringLiteral("handleNotificationAction('med_taken:demo-med-1') should return true"));
    }
    bool skipOk = false;
    QMetaObject::invokeMethod(
        ctrl,
        "handleNotificationAction",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, skipOk),
        Q_ARG(QString, QStringLiteral("med_skip:demo-med-2"))
    );
    if (!skipOk) {
        violations.append(QStringLiteral("handleNotificationAction('med_skip:demo-med-2') should return true"));
    }

    // State D: Inspect OverlayWidget (276x88 and minimum 240x80) across dark/color modes,
    // verify screen clamping, placement persistence, and tray icon hiding when only overlay is showing
    QMetaObject::invokeMethod(
        ctrl,
        "updateOverlayVisible",
        Qt::DirectConnection,
        Q_ARG(bool, true)
    );
    librelinkup_uitest::pump_events(50);
    if (QQuickWindow* ovWin = qobject_cast<QQuickWindow*>(librelinkup::find_overlay_window())) {
        for (bool darkOv : {true, false}) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateOverlayDarkTheme",
                Qt::DirectConnection,
                Q_ARG(bool, darkOv)
            );
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateOverlayOpacityPercent",
                Qt::DirectConnection,
                Q_ARG(int, darkOv ? 65 : 90)
            );
            sync_all();
            for (const QSize& ovSz : {QSize(276, 88), QSize(240, 80)}) {
                ovWin->resize(ovSz);
                librelinkup_uitest::pump_events(40);
                librelinkup_uitest::inspect_item_tree(
                    ovWin->contentItem(),
                    ovWin,
                    QStringLiteral("OverlayWidget(dark=%1) %2x%3").arg(darkOv ? 1 : 0).arg(ovSz.width()).arg(ovSz.height()),
                    violations
                );
            }
        }

        // Verify screen bounds clamping prevents overlay window from overflowing the screen
        librelinkup::restore_overlay_placement(QStringLiteral(""), 99999, 99999, 276, 88);
        librelinkup_uitest::pump_events(30);
        if (QScreen* sc = ovWin->screen()) {
            const QRect avail = sc->availableGeometry().isValid() ? sc->availableGeometry() : sc->geometry();
            if (ovWin->x() + ovWin->width() > avail.right() + 2 ||
                ovWin->y() + ovWin->height() > avail.bottom() + 2 ||
                ovWin->x() < avail.left() - 2 ||
                ovWin->y() < avail.top() - 2) {
                violations.append(
                    QStringLiteral("OverlayWidget (%1,%2 %3x%4) overflowed screen availableGeometry (%5,%6 %7x%8)")
                        .arg(ovWin->x()).arg(ovWin->y()).arg(ovWin->width()).arg(ovWin->height())
                        .arg(avail.x()).arg(avail.y()).arg(avail.width()).arg(avail.height())
                );
            }
        }

        // Verify exact coordinate restoration on screen
        librelinkup::restore_overlay_placement(QStringLiteral(""), 140, 64, 276, 88);
        librelinkup_uitest::pump_events(30);
        if (QScreen* sc = ovWin->screen()) {
            const QRect scGeom = sc->geometry();
            if (std::abs((ovWin->x() - scGeom.x()) - 140) > 2 ||
                std::abs((ovWin->y() - scGeom.y()) - 64) > 2) {
                violations.append(
                    QStringLiteral("OverlayWidget failed to restore exact relative position (140,64); got (%1,%2)")
                        .arg(ovWin->x() - scGeom.x())
                        .arg(ovWin->y() - scGeom.y())
                );
            }
        }

        // Verify overlay window uses Qt::Tool + Qt::BypassWindowManagerHint and has no transientParent
        // so it never creates a KDE Plasma Task Manager entry
        if ((ovWin->flags() & Qt::Tool) != Qt::Tool ||
            (ovWin->flags() & Qt::BypassWindowManagerHint) != Qt::BypassWindowManagerHint) {
            violations.append(QStringLiteral("OverlayWidget must have Qt::Tool and Qt::BypassWindowManagerHint flags so it never shows an icon in the KDE Plasma Task Manager"));
        }
        if (ovWin->transientParent() != nullptr) {
            violations.append(QStringLiteral("OverlayWidget must have null transientParent so it stays independent of MainWindow"));
        }

        // Verify On All Desktops toggle updates window flags and title
        QMetaObject::invokeMethod(ctrl, "updateOverlayOnAllDesktops", Qt::DirectConnection, Q_ARG(bool, false));
        sync_all();
        librelinkup_uitest::pump_events(30);
        if (settingsCtrl->property("overlayOnAllDesktops").toBool()) {
            violations.append(QStringLiteral("overlayOnAllDesktops should be false after updateOverlayOnAllDesktops(false)"));
        }
        if ((ovWin->flags() & Qt::BypassWindowManagerHint) != 0) {
            violations.append(QStringLiteral("OverlayWidget should clear Qt::BypassWindowManagerHint when overlayOnAllDesktops is false"));
        }
        QMetaObject::invokeMethod(ctrl, "updateOverlayOnAllDesktops", Qt::DirectConnection, Q_ARG(bool, true));
        sync_all();
        librelinkup_uitest::pump_events(30);
        if (!settingsCtrl->property("overlayOnAllDesktops").toBool() ||
            (ovWin->flags() & Qt::BypassWindowManagerHint) != Qt::BypassWindowManagerHint) {
            violations.append(QStringLiteral("OverlayWidget should restore Qt::BypassWindowManagerHint when overlayOnAllDesktops is true"));
        }

        // Verify when ONLY the overlay is showing (main window hidden + overlay visible):
        // - Main window is hidden (no Task Manager icon)
        // - Overlay remains visible on screen (with Qt::Tool, so no Task Manager icon)
        // - System tray icon ALWAYS remains visible
        QMetaObject::invokeMethod(ctrl, "hideMainWindow", Qt::DirectConnection);
        sync_all();
        librelinkup_uitest::pump_events(40);
        if (mainWin->isVisible()) {
            violations.append(QStringLiteral("MainWindow should be hidden (not in Task Manager) when minimized/closed to tray"));
        }
        if (!ovWin->isVisible()) {
            violations.append(QStringLiteral("OverlayWidget should remain visible when MainWindow is hidden"));
        }
        if (!librelinkup::TrayManager::instance().isTrayIconVisible()) {
            violations.append(QStringLiteral("Tray icon must always remain visible when only the overlay is showing"));
        }
        if (!settingsCtrl->property("overlayOnlyMode").toBool()) {
            violations.append(QStringLiteral("overlayOnlyMode should be true when main window is hidden while overlay is visible"));
        }

        // Verify tray icon remains visible when main window is shown again
        QMetaObject::invokeMethod(ctrl, "showMainWindow", Qt::DirectConnection);
        librelinkup_uitest::pump_events(40);
        if (!librelinkup::TrayManager::instance().isTrayIconVisible()) {
            violations.append(QStringLiteral("Tray icon should be visible when main window is shown"));
        }

        // Verify main window placement:
        // 1) When no saved screen position exists (relX=-1, relY=-1), main window centers on primary screen.
        librelinkup::restore_main_window_placement(QStringLiteral(""), -1, -1, 980, 680);
        librelinkup_uitest::pump_events(30);
        if (QScreen* prim = QGuiApplication::primaryScreen()) {
            const QRect avail = prim->availableGeometry().isValid() ? prim->availableGeometry() : prim->geometry();
            const int expectedX = avail.left() + (avail.width() - mainWin->width()) / 2;
            const int expectedY = avail.top() + (avail.height() - mainWin->height()) / 2;
            if (std::abs(mainWin->x() - expectedX) > 2 || std::abs(mainWin->y() - expectedY) > 2) {
                violations.append(
                    QStringLiteral("MainWindow with no saved position should center on primary screen at (%1,%2); got (%3,%4)")
                        .arg(expectedX).arg(expectedY).arg(mainWin->x()).arg(mainWin->y())
                );
            }
        }

        // 2) When a saved screen position exists, main window restores to that position (clamped to screen bounds).
        librelinkup::restore_main_window_placement(QStringLiteral(""), 14, 40, 780, 540);
        librelinkup_uitest::pump_events(30);
        if (QScreen* sc = mainWin->screen()) {
            const QRect scGeom = sc->geometry();
            if (std::abs((mainWin->x() - scGeom.x()) - 14) > 2 ||
                std::abs((mainWin->y() - scGeom.y()) - 40) > 2) {
                violations.append(
                    QStringLiteral("MainWindow failed to restore saved position (14,40); got (%1,%2)")
                        .arg(mainWin->x() - scGeom.x()).arg(mainWin->y() - scGeom.y())
                );
            }
        }

        // 3) Verify minimize-to-tray button (buttonToTray) never overflows mainHeaderToolBar or mainWin
        // when restored from tray across minimum, default, and large window sizes.
        for (const QSize& restoreSz : testSizes) {
            mainWin->resize(restoreSz);
            librelinkup_uitest::pump_events(30);
            QMetaObject::invokeMethod(settingsCtrl, "hideMainWindow", Qt::DirectConnection);
            librelinkup_uitest::pump_events(30);
            QMetaObject::invokeMethod(settingsCtrl, "showMainWindow", Qt::DirectConnection);
            librelinkup_uitest::pump_events(60);

            QQuickItem* btnTray = mainWin->findChild<QQuickItem*>(QStringLiteral("buttonToTray"));
            if (!btnTray || !btnTray->isVisible()) {
                violations.append(
                    QStringLiteral("[RestoreFromTray %1x%2] buttonToTray is missing or not visible")
                        .arg(restoreSz.width()).arg(restoreSz.height())
                );
            } else {
                const QPointF topLeft = btnTray->mapToScene(QPointF(0, 0));
                const qreal rightEdge = topLeft.x() + btnTray->width();
                if (topLeft.x() < 0.0 || rightEdge > mainWin->width() + 1.0) {
                    violations.append(
                        QStringLiteral("[RestoreFromTray %1x%2] buttonToTray [x=%3..%4] overflows window width (%5)")
                            .arg(restoreSz.width())
                            .arg(restoreSz.height())
                            .arg(topLeft.x(), 0, 'f', 1)
                            .arg(rightEdge, 0, 'f', 1)
                            .arg(mainWin->width())
                    );
                }
            }
            librelinkup_uitest::inspect_window_tree(
                mainWin,
                QStringLiteral("RestoreFromTray %1x%2").arg(restoreSz.width()).arg(restoreSz.height()),
                violations
            );
        }

        ovWin->hide();
    }

    // State E: Verify desktop alert notification snooze actions ("snooze_30", "snooze_60", "snooze_120")
    for (const QString& actKey : {
             QStringLiteral("snooze_30"),
             QStringLiteral("snooze_60"),
             QStringLiteral("snooze_120")}) {
        QMetaObject::invokeMethod(ctrl, "snoozeAlerts", Qt::DirectConnection, Q_ARG(int, 0));
        librelinkup_uitest::pump_events(20);
        if (ctrl->property("alertsSnoozed").toBool()) {
            violations.append(QStringLiteral("alertsSnoozed should be false after snoozeAlerts(0)"));
        }
        bool handled = false;
        QMetaObject::invokeMethod(
            ctrl,
            "handleNotificationAction",
            Qt::DirectConnection,
            Q_RETURN_ARG(bool, handled),
            Q_ARG(QString, actKey)
        );
        librelinkup_uitest::pump_events(20);
        if (!handled) {
            violations.append(
                QStringLiteral("handleNotificationAction('%1') returned false").arg(actKey)
            );
        }
        if (!ctrl->property("alertsSnoozed").toBool() ||
            ctrl->property("snoozeUntilDisplay").toString().isEmpty()) {
            violations.append(
                QStringLiteral("alertsSnoozed/snoozeUntilDisplay not updated after '%1'").arg(actKey)
            );
        }
    }
    QMetaObject::invokeMethod(ctrl, "snoozeAlerts", Qt::DirectConnection, Q_ARG(int, 0));
    librelinkup_uitest::pump_events(20);

    // State F: Verify Wide High-Contrast Scrollbars & Ploopy High-Resolution Wheel Scrolling
    {
        mainWin->resize(780, 540);
        mainWin->show();
        librelinkup_uitest::pump_events(40);

        const QStringList scrollBarNames = {
            QStringLiteral("loginVerticalScrollBar"),
            QStringLiteral("medicationsVerticalScrollBar"),
            QStringLiteral("generalVerticalScrollBar"),
            QStringLiteral("alertsVerticalScrollBar"),
            QStringLiteral("accountVerticalScrollBar"),
            QStringLiteral("logVerticalScrollBar"),
            QStringLiteral("aboutVerticalScrollBar")
        };
        for (const QString& sbName : scrollBarNames) {
            QQuickItem* sb = mainWin->findChild<QQuickItem*>(sbName);
            if (!sb) {
                violations.append(QStringLiteral("Missing HighContrastScrollBar '%1'").arg(sbName));
                continue;
            }
            if (sb->implicitWidth() < 11.5) {
                violations.append(
                    QStringLiteral("HighContrastScrollBar '%1' implicitWidth (%2) should be >= 12px")
                        .arg(sbName).arg(sb->implicitWidth(), 0, 'f', 1)
                );
            }
            if (sb->property("minimumSize").toReal() < 0.13) {
                violations.append(
                    QStringLiteral("HighContrastScrollBar '%1' minimumSize (%2) should be >= 0.14")
                        .arg(sbName).arg(sb->property("minimumSize").toReal(), 0, 'f', 2)
                );
            }
        }

        // Test 1: Ploopy micro-step wheel scrolling & visible scrollbar on medicationsScrollView (780x540)
        mainWin->setProperty("activeTab", QStringLiteral("medications"));
        sync_all();
        librelinkup_uitest::pump_events(40);
        if (QQuickItem* medScroll = mainWin->findChild<QQuickItem*>(QStringLiteral("medicationsScrollView"))) {
            QQuickItem* medFlick = medScroll->inherits("QQuickFlickable")
                ? medScroll
                : qobject_cast<QQuickItem*>(medScroll->property("contentItem").value<QObject*>());
            if (!medFlick) {
                violations.append(QStringLiteral("medicationsScrollView is not a QQuickFlickable"));
            } else {
                if (QQuickItem* medSb = mainWin->findChild<QQuickItem*>(QStringLiteral("medicationsVerticalScrollBar"))) {
                    const qreal sbSize = medSb->property("size").toReal();
                    QQuickItem* sbBg = qobject_cast<QQuickItem*>(medSb->property("background").value<QObject*>());
                    QQuickItem* sbThumb = qobject_cast<QQuickItem*>(medSb->property("contentItem").value<QObject*>());
                    if (!medSb->isVisible() || medSb->width() < 11.5 || medSb->height() < 100.0 || sbSize >= 1.0) {
                        violations.append(
                            QStringLiteral("medicationsVerticalScrollBar must be visible with width>=12, height>=100, size<1.0 (got vis=%1, w=%2, h=%3, size=%4)")
                                .arg(medSb->isVisible() ? 1 : 0)
                                .arg(medSb->width(), 0, 'f', 1)
                                .arg(medSb->height(), 0, 'f', 1)
                                .arg(sbSize, 0, 'f', 3)
                        );
                    }
                    if (!sbBg || !sbBg->isVisible() || !sbThumb || !sbThumb->isVisible()) {
                        violations.append(QStringLiteral("medicationsVerticalScrollBar track and thumb must be visible when content overflows"));
                    }
                }
                medFlick->setProperty("contentY", 0.0);
                librelinkup_uitest::pump_events(20);
                const QPointF centerScene = medScroll->mapToScene(QPointF(medScroll->width() * 0.5, medScroll->height() * 0.5));
                const QPointF globalPt = mainWin->mapToGlobal(centerScene.toPoint());
                // Simulate single Ploopy hi-res micro-step down: angleDelta.y = -12 -> +6.0px contentY
                QWheelEvent microDown(
                    centerScene,
                    globalPt,
                    QPoint(0, 0),
                    QPoint(0, -12),
                    Qt::NoButton,
                    Qt::NoModifier,
                    Qt::NoScrollPhase,
                    false
                );
                QCoreApplication::sendEvent(mainWin, &microDown);
                librelinkup_uitest::pump_events(15);
                const qreal afterDownY = medFlick->property("contentY").toReal();
                if (std::abs(afterDownY - 6.0) > 0.5) {
                    violations.append(
                        QStringLiteral("Ploopy micro-step wheel down on medicationsScrollView expected contentY=6.0, got %1")
                            .arg(afterDownY, 0, 'f', 2)
                    );
                }
            }
        }

        // Test 2: Ploopy micro-step wheel scrolling on logListView (logs tab) + scroll preservation on refresh
        // Generate enough log entries by toggling demo / refresh logs so logListView is scrollable at 780x540
        for (int i = 0; i < 18; ++i) {
            QMetaObject::invokeMethod(ctrl, "loadDemoSampleData", Qt::DirectConnection);
        }
        mainWin->setProperty("activeTab", QStringLiteral("logs"));
        sync_all();
        librelinkup_uitest::pump_events(50);

        if (QQuickItem* logList = mainWin->findChild<QQuickItem*>(QStringLiteral("logListView"))) {
            const qreal contentH = logList->property("contentHeight").toReal();
            const qreal viewH = logList->height();
            if (contentH <= viewH + 10.0) {
                violations.append(
                    QStringLiteral("logListView should be scrollable in test (contentHeight=%1, height=%2)")
                        .arg(contentH, 0, 'f', 1).arg(viewH, 0, 'f', 1)
                );
            } else {
                const qreal initialBottomY = logList->property("contentY").toReal();
                const QPointF centerScene = logList->mapToScene(QPointF(logList->width() * 0.5, logList->height() * 0.5));
                const QPointF globalPt = mainWin->mapToGlobal(centerScene.toPoint());
                // Simulate Ploopy hi-res micro-step up: angleDelta.y = +16 -> -8.0px contentY
                QWheelEvent microUp(
                    centerScene,
                    globalPt,
                    QPoint(0, 0),
                    QPoint(0, 16),
                    Qt::NoButton,
                    Qt::NoModifier,
                    Qt::NoScrollPhase,
                    false
                );
                QCoreApplication::sendEvent(mainWin, &microUp);
                librelinkup_uitest::pump_events(15);
                const qreal afterUpY = logList->property("contentY").toReal();
                if (std::abs((initialBottomY - afterUpY) - 8.0) > 0.5) {
                    violations.append(
                        QStringLiteral("Ploopy micro-step wheel up on logListView expected delta=-8.0, got %1 -> %2")
                            .arg(initialBottomY, 0, 'f', 2).arg(afterUpY, 0, 'f', 2)
                    );
                }

                // Scroll up to top (contentY = 0) and verify background log event does NOT reset scroll to bottom
                logList->setProperty("contentY", 0.0);
                librelinkup_uitest::pump_events(15);
                QMetaObject::invokeMethod(ctrl, "loadDemoSampleData", Qt::DirectConnection);
                QMetaObject::invokeMethod(settingsCtrl, "refreshEventLogs", Qt::DirectConnection);
                sync_all();
                librelinkup_uitest::pump_events(30);
                const qreal afterRefreshY = logList->property("contentY").toReal();
                if (afterRefreshY > 5.0) {
                    violations.append(
                        QStringLiteral("logListView scroll position was yanked from 0.0 to %1 on background log update")
                            .arg(afterRefreshY, 0, 'f', 1)
                    );
                }
            }
        }
        mainWin->setProperty("activeTab", QStringLiteral("dashboard"));
        librelinkup_uitest::pump_events(25);
    }

    const QString screenshotDir = qEnvironmentVariable("LLU_EXPORT_SCREENSHOTS_DIR").trimmed();
    if (!screenshotDir.isEmpty()) {
        QDir dir(screenshotDir);
        dir.mkpath(QStringLiteral("."));

        // 1. Main Window - Dashboard (Dark theme, 980x680, with interactive graph point hover active)
        QMetaObject::invokeMethod(ctrl, "loadDemoSampleData", Qt::DirectConnection);
        QMetaObject::invokeMethod(ctrl, "updateThemeMode", Qt::DirectConnection, Q_ARG(QString, QStringLiteral("dark")));
        sync_all();
        mainWin->resize(980, 680);
        mainWin->setProperty("activeTab", QStringLiteral("dashboard"));
        mainWin->show();
        librelinkup_uitest::pump_events(80);
        if (QQuickItem* canvas = mainWin->findChild<QQuickItem*>(QStringLiteral("glucoseCanvas"))) {
            QMetaObject::invokeMethod(canvas, "requestPaint", Qt::DirectConnection);
            QMetaObject::invokeMethod(
                canvas,
                "updateHoverAt",
                Qt::DirectConnection,
                Q_ARG(QVariant, QVariant(canvas->width() * 0.58)),
                Q_ARG(QVariant, QVariant(canvas->height() * 0.45))
            );
            librelinkup_uitest::pump_events(60);
        }
        QImage dashImg = mainWin->grabWindow();
        dashImg.save(dir.filePath(QStringLiteral("app-dashboard.png")));

        // 2. Main Window - Medications & Glycemic Analytics (Dark theme, 980x880 to showcase full tab)
        mainWin->resize(980, 880);
        mainWin->setProperty("activeTab", QStringLiteral("medications"));
        sync_all();
        librelinkup_uitest::pump_events(80);
        QImage medsImg = mainWin->grabWindow();
        medsImg.save(dir.filePath(QStringLiteral("app-medications.png")));
        mainWin->resize(980, 680);

        // 3. Main Window - General, Tray & Overlay Settings (Dark theme, 980x680)
        mainWin->setProperty("activeTab", QStringLiteral("general"));
        sync_all();
        librelinkup_uitest::pump_events(60);
        QImage settingsImg = mainWin->grabWindow();
        settingsImg.save(dir.filePath(QStringLiteral("app-settings.png")));

        // 4. Desktop Overlay Widget (Dark Slate Theme & Classic Status-Color Theme side-by-side)
        if (QQuickWindow* ovWin = qobject_cast<QQuickWindow*>(librelinkup::find_overlay_window())) {
            QMetaObject::invokeMethod(ctrl, "updateOverlayVisible", Qt::DirectConnection, Q_ARG(bool, true));
            QMetaObject::invokeMethod(ctrl, "updateOverlayOnAllDesktops", Qt::DirectConnection, Q_ARG(bool, true));
            ovWin->resize(276, 88);

            QMetaObject::invokeMethod(settingsCtrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, true));
            QMetaObject::invokeMethod(settingsCtrl, "updateOverlayOpacityPercent", Qt::DirectConnection, Q_ARG(int, 92));
            sync_all();
            librelinkup_uitest::pump_events(60);
            QImage ovDarkImg = ovWin->grabWindow();

            QMetaObject::invokeMethod(settingsCtrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, false));
            QMetaObject::invokeMethod(settingsCtrl, "updateOverlayOpacityPercent", Qt::DirectConnection, Q_ARG(int, 95));
            sync_all();
            librelinkup_uitest::pump_events(60);
            QImage ovColorImg = ovWin->grabWindow();

            QImage ovComposite(620, 148, QImage::Format_ARGB32_Premultiplied);
            ovComposite.fill(QColor(15, 23, 42));
            {
                QPainter p(&ovComposite);
                p.setRenderHint(QPainter::Antialiasing, true);
                p.setRenderHint(QPainter::TextAntialiasing, true);

                QFont labelFont = p.font();
                labelFont.setPixelSize(12);
                labelFont.setBold(true);
                p.setFont(labelFont);
                p.setPen(QColor(203, 213, 225));
                p.drawText(QRect(24, 10, 276, 20), Qt::AlignCenter, QStringLiteral("Overlay — Dark Slate Theme"));
                p.drawText(QRect(320, 10, 276, 20), Qt::AlignCenter, QStringLiteral("Overlay — Status Color Theme"));

                p.drawImage(24, 38, ovDarkImg.scaled(276, 88, Qt::IgnoreAspectRatio, Qt::SmoothTransformation));
                p.drawImage(320, 38, ovColorImg.scaled(276, 88, Qt::IgnoreAspectRatio, Qt::SmoothTransformation));
            }
            ovComposite.save(dir.filePath(QStringLiteral("overlay-widget.png")));
            ovWin->hide();
        }

        // 4. System Tray Icons Showcase (Default SVG Drop Icons + Dynamic Glucose Value Badges)
        {
            QImage trayComposite(660, 152, QImage::Format_ARGB32_Premultiplied);
            trayComposite.fill(QColor(15, 23, 42));
            QPainter p(&trayComposite);
            p.setRenderHint(QPainter::Antialiasing, true);
            p.setRenderHint(QPainter::TextAntialiasing, true);
            p.setRenderHint(QPainter::SmoothPixmapTransform, true);

            struct TraySample {
                QIcon icon;
                QString label;
                QColor panelBg;
            };
            const QList<TraySample> samples = {
                {
                    librelinkup::build_multisize_svg_icon(QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")),
                    QStringLiteral("Dark Panel Icon"),
                    QColor(30, 41, 59)
                },
                {
                    librelinkup::build_multisize_svg_icon(QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-light.svg")),
                    QStringLiteral("Light Panel Icon"),
                    QColor(226, 232, 240)
                },
                {
                    librelinkup::build_glucose_badge_icon(QStringLiteral("118"), QStringLiteral("→"), QStringLiteral("#22c55e")),
                    QStringLiteral("In Range (118 →)"),
                    QColor(30, 41, 59)
                },
                {
                    librelinkup::build_glucose_badge_icon(QStringLiteral("214"), QStringLiteral("↑"), QStringLiteral("#f59e0b")),
                    QStringLiteral("High (214 ↑)"),
                    QColor(30, 41, 59)
                },
                {
                    librelinkup::build_glucose_badge_icon(QStringLiteral("62"), QStringLiteral("↓"), QStringLiteral("#ef4444")),
                    QStringLiteral("Low (62 ↓)"),
                    QColor(30, 41, 59)
                }
            };

            QFont f = p.font();
            f.setPixelSize(11);
            f.setBold(true);
            p.setFont(f);

            const int slotW = 124;
            const int startX = 20;
            for (int i = 0; i < samples.size(); ++i) {
                const int cx = startX + i * slotW;
                QRectF tileRect(cx + 22, 18, 80, 80);
                p.setPen(QPen(QColor(148, 163, 184, 70), 1.0));
                p.setBrush(samples[i].panelBg);
                p.drawRoundedRect(tileRect, 14, 14);

                QPixmap pm = samples[i].icon.pixmap(64, 64);
                p.drawPixmap(cx + 30, 26, 64, 64, pm);

                p.setPen(QColor(226, 232, 240));
                p.drawText(QRect(cx, 108, slotW, 28), Qt::AlignCenter, samples[i].label);
            }
            p.end();
            trayComposite.save(dir.filePath(QStringLiteral("tray-icons.png")));
        }
    }

    const QString videoFramesDir = qEnvironmentVariable("LLU_EXPORT_VIDEO_FRAMES_DIR").trimmed();
    if (!videoFramesDir.isEmpty()) {
        QDir vdir(videoFramesDir);
        vdir.mkpath(QStringLiteral("."));

        QMetaObject::invokeMethod(ctrl, "loadDemoSampleData", Qt::DirectConnection);
        QMetaObject::invokeMethod(ctrl, "updateThemeMode", Qt::DirectConnection, Q_ARG(QString, QStringLiteral("dark")));
        QMetaObject::invokeMethod(ctrl, "updateOverlayVisible", Qt::DirectConnection, Q_ARG(bool, true));
        QMetaObject::invokeMethod(ctrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, true));
        QMetaObject::invokeMethod(ctrl, "updateOverlayOpacityPercent", Qt::DirectConnection, Q_ARG(int, 92));

        mainWin->resize(980, 660);
        mainWin->setProperty("activeTab", QStringLiteral("dashboard"));
        mainWin->show();
        librelinkup_uitest::pump_events(80);

        QQuickWindow* ovWin = qobject_cast<QQuickWindow*>(librelinkup::find_overlay_window());
        QQuickWindow* logWin = qobject_cast<QQuickWindow*>(librelinkup::find_log_viewer_window());
        QQuickWindow* aboutWin = qobject_cast<QQuickWindow*>(librelinkup::find_about_window());
        QQuickItem* canvas = mainWin->findChild<QQuickItem*>(QStringLiteral("glucoseCanvas"));

        // Capture key QML window snapshots
        QVector<QImage> hoverDashFrames;
        if (canvas) {
            QMetaObject::invokeMethod(canvas, "clearHover", Qt::DirectConnection);
            librelinkup_uitest::pump_events(30);
        }
        QImage dashBaseDark = mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

        const int hoverSteps = 15;
        for (int s = 0; s < hoverSteps; ++s) {
            if (canvas) {
                const qreal ratio = static_cast<qreal>(s) / static_cast<qreal>(hoverSteps - 1);
                const qreal rx = 56.0 + ratio * (canvas->width() - 84.0);
                QMetaObject::invokeMethod(
                    canvas,
                    "updateHoverAt",
                    Qt::DirectConnection,
                    Q_ARG(QVariant, QVariant(rx)),
                    Q_ARG(QVariant, QVariant(canvas->height() * 0.48))
                );
                librelinkup_uitest::pump_events(25);
            }
            hoverDashFrames.append(mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation));
        }
        if (canvas) {
            QMetaObject::invokeMethod(canvas, "clearHover", Qt::DirectConnection);
        }

        // Light mode dashboard snapshot
        QMetaObject::invokeMethod(ctrl, "updateThemeMode", Qt::DirectConnection, Q_ARG(QString, QStringLiteral("light")));
        librelinkup_uitest::pump_events(60);
        QImage dashLight = mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

        // Restore Dark mode for Settings, Alerts, Account, LogViewer, and About snapshots
        QMetaObject::invokeMethod(ctrl, "updateThemeMode", Qt::DirectConnection, Q_ARG(QString, QStringLiteral("dark")));
        librelinkup_uitest::pump_events(60);

        mainWin->setProperty("activeTab", QStringLiteral("general"));
        librelinkup_uitest::pump_events(50);
        QImage tabGeneralImg = mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

        mainWin->setProperty("activeTab", QStringLiteral("alerts"));
        librelinkup_uitest::pump_events(50);
        QImage tabAlertsImg = mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

        mainWin->setProperty("activeTab", QStringLiteral("account"));
        librelinkup_uitest::pump_events(50);
        QImage tabAccountImg = mainWin->grabWindow().scaled(980, 660, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

        mainWin->setProperty("activeTab", QStringLiteral("dashboard"));
        librelinkup_uitest::pump_events(40);

        QImage ovDarkImg(276, 88, QImage::Format_ARGB32_Premultiplied);
        QImage ovColorImg(276, 88, QImage::Format_ARGB32_Premultiplied);
        if (ovWin) {
            ovWin->resize(276, 88);
            QMetaObject::invokeMethod(ctrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, true));
            librelinkup_uitest::pump_events(40);
            ovDarkImg = ovWin->grabWindow().scaled(276, 88, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

            QMetaObject::invokeMethod(ctrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, false));
            librelinkup_uitest::pump_events(40);
            ovColorImg = ovWin->grabWindow().scaled(276, 88, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

            QMetaObject::invokeMethod(ctrl, "updateOverlayDarkTheme", Qt::DirectConnection, Q_ARG(bool, true));
            ovWin->hide();
        }

        QImage logDialogImg;
        if (logWin) {
            QMetaObject::invokeMethod(logWin, "openLogWindow", Qt::DirectConnection);
            logWin->resize(720, 460);
            librelinkup_uitest::pump_events(50);
            logDialogImg = logWin->grabWindow().scaled(720, 460, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
            logWin->hide();
        }

        QImage aboutDialogImg;
        if (aboutWin) {
            QMetaObject::invokeMethod(aboutWin, "openAboutWindow", Qt::DirectConnection);
            aboutWin->resize(580, 500);
            librelinkup_uitest::pump_events(50);
            aboutDialogImg = aboutWin->grabWindow().scaled(580, 500, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
            aboutWin->hide();
        }

        QPixmap trayBadgePm = librelinkup::build_glucose_badge_icon(
            QStringLiteral("112"),
            QStringLiteral("→"),
            QStringLiteral("#22c55e")
        ).pixmap(28, 28);
        QPixmap appIconPm = librelinkup::build_multisize_svg_icon(
            QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
        ).pixmap(24, 24);

        const int totalFrames = 360; // 12 seconds @ 30fps
        for (int f = 0; f < totalFrames; ++f) {
            QImage desktop(1280, 800, QImage::Format_RGB32);
            QPainter p(&desktop);
            p.setRenderHint(QPainter::Antialiasing, true);
            p.setRenderHint(QPainter::TextAntialiasing, true);
            p.setRenderHint(QPainter::SmoothPixmapTransform, true);

            // KDE Plasma 6 desktop wallpaper gradient
            QLinearGradient bgGrad(0, 0, 1280, 756);
            bgGrad.setColorAt(0.0, QColor(15, 23, 42));
            bgGrad.setColorAt(0.5, QColor(23, 37, 84));
            bgGrad.setColorAt(1.0, QColor(15, 118, 110));
            p.fillRect(0, 0, 1280, 756, bgGrad);

            bool showMainWin = (f < 338);
            bool useColorOv = (f >= 100 && f < 135);
            QPoint ovPos(984, 24);
            if (f >= 105 && f < 135) {
                const double t = static_cast<double>(f - 105) / 29.0;
                const int dy = static_cast<int>(std::sin(t * 3.14159265) * 90.0);
                ovPos = QPoint(984 - dy / 2, 24 + dy);
            } else if (!showMainWin) {
                ovPos = QPoint(960, 36);
            }

            QPointF cursorPos(640, 400);
            QString bannerCaption;

            if (showMainWin) {
                // Window drop shadow & frame
                p.setPen(Qt::NoPen);
                p.setBrush(QColor(0, 0, 0, 110));
                p.drawRoundedRect(QRectF(42, 34, 988, 668), 12, 12);

                QImage currentMain = dashBaseDark;
                if (f < 15) {
                    currentMain = dashBaseDark;
                    cursorPos = QPointF(260 + f * 8, 220 + f * 12);
                    bannerCaption = QStringLiteral("Live Blood Glucose Dashboard & KDE System Tray Badge");
                } else if (f < 90) {
                    const int idx = std::clamp(((f - 15) * hoverSteps) / 75, 0, hoverSteps - 1);
                    currentMain = hoverDashFrames.value(idx, dashBaseDark);
                    const double ratio = static_cast<double>(f - 15) / 74.0;
                    cursorPos = QPointF(120.0 + ratio * 820.0, 485.0);
                    bannerCaption = QStringLiteral("12-Hour Interactive Glucose Graph — Point Hover Inspection");
                } else if (f < 135) {
                    currentMain = dashBaseDark;
                    cursorPos = QPointF(ovPos.x() + 140, ovPos.y() + 44);
                    bannerCaption = QStringLiteral("Floating Desktop Overlay — Dark Slate & Status Color Themes + Multi-Monitor Drag");
                } else if (f < 165) {
                    currentMain = dashLight;
                    cursorPos = QPointF(532, 270);
                    bannerCaption = QStringLiteral("Native KDE Plasma 6 Breeze Light & Breeze Dark Theme Support");
                } else if (f < 180) {
                    currentMain = dashBaseDark;
                    cursorPos = QPointF(480, 270);
                    bannerCaption = QStringLiteral("Native KDE Plasma 6 Breeze Light & Breeze Dark Theme Support");
                } else if (f < 225) {
                    currentMain = tabGeneralImg;
                    cursorPos = QPointF(492, 54);
                    bannerCaption = QStringLiteral("General, System Tray Badge & Floating Overlay Transparency Settings");
                } else if (f < 248) {
                    currentMain = tabAlertsImg;
                    cursorPos = QPointF(580, 54);
                    bannerCaption = QStringLiteral("Customizable High/Low Glucose Thresholds & Audio/Window Alerts");
                } else if (f < 270) {
                    currentMain = tabAccountImg;
                    cursorPos = QPointF(668, 54);
                    bannerCaption = QStringLiteral("AES-256-GCM + PBKDF2-HMAC-SHA256 Encrypted Credential Vault");
                } else {
                    currentMain = dashBaseDark;
                }

                p.drawImage(46, 30, currentMain);

                if (f >= 270 && f < 315 && !logDialogImg.isNull()) {
                    p.setBrush(QColor(0, 0, 0, 140));
                    p.drawRoundedRect(QRectF(172, 134, 728, 468), 12, 12);
                    p.drawImage(176, 130, logDialogImg);
                    cursorPos = QPointF(520, 260);
                    bannerCaption = QStringLiteral("Built-In Diagnostic Log Viewer — Category & Severity Filtering + Export");
                } else if (f >= 315 && f < 338 && !aboutDialogImg.isNull()) {
                    p.setBrush(QColor(0, 0, 0, 140));
                    p.drawRoundedRect(QRectF(242, 114, 588, 508), 12, 12);
                    p.drawImage(246, 110, aboutDialogImg);
                    cursorPos = QPointF(536, 320);
                    bannerCaption = QStringLiteral("About LibreLinkUp Rust Desktop v0.2.0 — Apache-2.0 License");
                }
            } else {
                cursorPos = QPointF(ovPos.x() + 120, ovPos.y() + 44);
                bannerCaption = QStringLiteral("Overlay-Only Mode — Floating Desktop Overlay + System Tray Badge (No Taskbar Clutter)");
            }

            // Draw Floating Desktop Overlay Widget
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(0, 0, 0, 110));
            p.drawRoundedRect(QRectF(ovPos.x() - 2, ovPos.y() + 3, 280, 92), 16, 16);
            p.drawImage(ovPos.x(), ovPos.y(), useColorOv ? ovColorImg : ovDarkImg);

            // Draw KDE Plasma 6 Bottom Panel (y = 756..800)
            p.fillRect(0, 756, 1280, 44, QColor(15, 23, 42, 238));
            p.setPen(QColor(51, 65, 85));
            p.drawLine(0, 756, 1280, 756);

            // KDE Launcher icon on left of panel
            p.setPen(Qt::NoPen);
            p.setBrush(QColor(29, 153, 243));
            p.drawRoundedRect(QRectF(12, 764, 28, 28), 6, 6);
            p.setPen(Qt::white);
            QFont kf = p.font();
            kf.setPixelSize(14);
            kf.setBold(true);
            p.setFont(kf);
            p.drawText(QRect(12, 764, 28, 28), Qt::AlignCenter, QStringLiteral("K"));

            // Task Manager entry ONLY when main window is open (never when only overlay is visible!)
            if (showMainWin) {
                p.setPen(QPen(QColor(56, 189, 248, 160), 1.0));
                p.setBrush(QColor(30, 41, 59));
                p.drawRoundedRect(QRectF(52, 762, 236, 32), 6, 6);
                p.drawPixmap(60, 766, 24, 24, appIconPm);
                QFont tf = p.font();
                tf.setPixelSize(12);
                tf.setBold(false);
                p.setFont(tf);
                p.setPen(QColor(226, 232, 240));
                p.drawText(QRect(92, 762, 188, 32), Qt::AlignVCenter | Qt::AlignLeft, QStringLiteral("LibreLinkUp Rust Desktop"));
            }

            // System Tray area on right of panel (ALWAYS shows the live glucose badge icon!)
            p.setPen(QPen(QColor(71, 85, 105), 1.0));
            p.setBrush(QColor(30, 41, 59));
            p.drawRoundedRect(QRectF(1124, 762, 144, 32), 6, 6);
            p.drawPixmap(1134, 764, 28, 28, trayBadgePm);
            QFont cf = p.font();
            cf.setPixelSize(12);
            cf.setBold(true);
            p.setFont(cf);
            p.setPen(QColor(226, 232, 240));
            p.drawText(QRect(1170, 762, 90, 32), Qt::AlignCenter, QStringLiteral("10:05 AM"));

            // Caption pill at bottom-left above panel
            if (!bannerCaption.isEmpty()) {
                QFont bf = p.font();
                bf.setPixelSize(13);
                bf.setBold(true);
                p.setFont(bf);
                p.setPen(QPen(QColor(56, 189, 248, 180), 1.5));
                p.setBrush(QColor(15, 23, 42, 225));
                p.drawRoundedRect(QRectF(46, 708, 680, 36), 8, 8);
                p.setPen(QColor(248, 250, 252));
                p.drawText(QRect(62, 708, 650, 36), Qt::AlignVCenter | Qt::AlignLeft, bannerCaption);
            }

            // Draw animated cursor pointer
            QPainterPath cursorPath;
            cursorPath.moveTo(cursorPos);
            cursorPath.lineTo(cursorPos.x(), cursorPos.y() + 18);
            cursorPath.lineTo(cursorPos.x() + 5, cursorPos.y() + 14);
            cursorPath.lineTo(cursorPos.x() + 9, cursorPos.y() + 21);
            cursorPath.lineTo(cursorPos.x() + 12, cursorPos.y() + 19);
            cursorPath.lineTo(cursorPos.x() + 8, cursorPos.y() + 12);
            cursorPath.lineTo(cursorPos.x() + 14, cursorPos.y() + 12);
            cursorPath.closeSubpath();
            p.setPen(QPen(QColor(15, 23, 42), 1.5));
            p.setBrush(Qt::white);
            p.drawPath(cursorPath);

            p.end();
            const QString frameName = QStringLiteral("frame_%1.png").arg(f, 4, 10, QLatin1Char('0'));
            desktop.save(vdir.filePath(frameName));
        }
    }

    mainWin->hide();
    if (violations.isEmpty()) {
        return QStringLiteral("OK");
    }
    return violations.join(QStringLiteral("\n"));
}
