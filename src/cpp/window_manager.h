#pragma once

#include <QCoreApplication>
#include <QCursor>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusReply>
#include <QDateTime>
#include <QDir>
#include <QFile>
#include <QFileDevice>
#include <QGuiApplication>
#include <QHoverEvent>
#include <QKeyEvent>
#include <QMetaObject>
#include <QMouseEvent>
#include <QObject>
#include <QPoint>
#include <QPointF>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRect>
#include <QScreen>
#include <QUuid>
#include <QStandardPaths>
#include <QString>
#include <QTimer>
#include <QVariant>
#include <QWheelEvent>
#include <QWindow>
#include <algorithm>
#include <cmath>
#include <cstdint>

namespace librelinkup {

inline QPointer<QWindow>& main_window_ref() {
    static QPointer<QWindow> win;
    return win;
}

inline QWindow* find_main_window();
inline QWindow* find_overlay_window();
inline void update_tray_window_actions();
inline void record_main_window_placement(QWindow* win, bool persistToSettings = true);

struct MainWindowPlacementState {
    QString screenName;
    int relX = -1;
    int relY = -1;
    int width = 980;
    int height = 680;
    bool hasPlacement = false;
    bool applying = false;

    static MainWindowPlacementState& instance() {
        static MainWindowPlacementState st;
        return st;
    }

    bool hasSavedPosition() const {
        return relX >= 0 && relY >= 0;
    }

    void applyToWindow(QWindow* win, bool preserveCurrentSize = false) {
        if (!win || applying) {
            return;
        }
        applying = true;
        QScreen* targetScreen = nullptr;
        if (hasSavedPosition()) {
            const QString cleanName = screenName.trimmed();
            if (!cleanName.isEmpty()) {
                for (QScreen* sc : QGuiApplication::screens()) {
                    if (sc && sc->name().compare(cleanName, Qt::CaseInsensitive) == 0) {
                        targetScreen = sc;
                        break;
                    }
                }
            }
            if (!targetScreen) {
                targetScreen = QGuiApplication::primaryScreen()
                    ? QGuiApplication::primaryScreen()
                    : win->screen();
            }
        } else {
            // First launch with no saved screen position: always center on the primary screen.
            targetScreen = QGuiApplication::primaryScreen()
                ? QGuiApplication::primaryScreen()
                : win->screen();
        }
        if (!targetScreen) {
            applying = false;
            return;
        }

        const QRect scGeom = targetScreen->geometry();
        const QRect avail = targetScreen->availableGeometry().isValid()
            ? targetScreen->availableGeometry()
            : scGeom;
        if (!avail.isValid()) {
            applying = false;
            return;
        }

        const int minW = std::max(780, win->minimumWidth());
        const int minH = std::max(540, win->minimumHeight());
        const int baseW = preserveCurrentSize
            ? std::max(minW, win->width())
            : (width >= minW ? width : std::max(minW, win->width()));
        const int baseH = preserveCurrentSize
            ? std::max(minH, win->height())
            : (height >= minH ? height : std::max(minH, win->height()));

        const int w = std::clamp(baseW, minW, std::max(minW, avail.width()));
        const int h = std::clamp(baseH, minH, std::max(minH, avail.height()));

        int globalX = avail.left() + (avail.width() - w) / 2;
        int globalY = avail.top() + (avail.height() - h) / 2;
        if (hasSavedPosition()) {
            globalX = scGeom.x() + relX;
            globalY = scGeom.y() + relY;
        }

        const int minX = avail.left();
        const int maxX = std::max(avail.left(), avail.right() - w + 1);
        const int minY = avail.top();
        const int maxY = std::max(avail.top(), avail.bottom() - h + 1);
        const int clampedX = std::clamp(globalX, minX, maxX);
        const int clampedY = std::clamp(globalY, minY, maxY);

        if (win->screen() != targetScreen) {
            win->setScreen(targetScreen);
        }
        if (win->width() != w || win->height() != h) {
            win->resize(w, h);
        }
        if (win->x() != clampedX || win->y() != clampedY) {
            win->setPosition(clampedX, clampedY);
        }
        applying = false;
    }
};

inline void ensure_window_hooks(QWindow* win) {
    if (!win || win->property("_llu_minimize_hooked").toBool()) {
        return;
    }
    win->setProperty("_llu_minimize_hooked", true);

    QTimer* geomSaveTimer = new QTimer(win);
    geomSaveTimer->setSingleShot(true);
    geomSaveTimer->setInterval(250);
    QObject::connect(geomSaveTimer, &QTimer::timeout, win, [win]() {
        if (win && win->isVisible() && !(win->windowStates() & Qt::WindowMinimized)
            && qEnvironmentVariable("QT_QPA_PLATFORM") != QStringLiteral("offscreen")) {
            record_main_window_placement(win, true);
        }
    });
    auto scheduleGeomSave = [win, geomSaveTimer]() {
        if (!win || MainWindowPlacementState::instance().applying) {
            return;
        }
        if (win->isVisible() && !(win->windowStates() & Qt::WindowMinimized)
            && qEnvironmentVariable("QT_QPA_PLATFORM") != QStringLiteral("offscreen")) {
            geomSaveTimer->start();
        }
    };
    QObject::connect(win, &QWindow::xChanged, win, [scheduleGeomSave](int) { scheduleGeomSave(); });
    QObject::connect(win, &QWindow::yChanged, win, [scheduleGeomSave](int) { scheduleGeomSave(); });
    QObject::connect(win, &QWindow::widthChanged, win, [scheduleGeomSave](int) { scheduleGeomSave(); });
    QObject::connect(win, &QWindow::heightChanged, win, [scheduleGeomSave](int) { scheduleGeomSave(); });

    // When minimized, hide to tray and clear WindowMinimized state so there is
    // only ever a single icon in the system tray (never duplicate tray/taskbar icons).
    QObject::connect(win, &QWindow::windowStateChanged, win, [win](Qt::WindowState state) {
        if (state & Qt::WindowMinimized) {
            QTimer::singleShot(0, win, [win]() {
                if (win) {
                    win->hide();
                    win->setWindowStates(win->windowStates() & ~Qt::WindowMinimized);
                    update_tray_window_actions();
                }
            });
        } else {
            update_tray_window_actions();
        }
    });

    QObject::connect(win, &QWindow::visibilityChanged, win, [win](QWindow::Visibility visibility) {
        if (visibility == QWindow::Minimized) {
            QTimer::singleShot(0, win, [win]() {
                if (win) {
                    win->hide();
                    win->setWindowStates(win->windowStates() & ~Qt::WindowMinimized);
                    update_tray_window_actions();
                }
            });
        } else {
            update_tray_window_actions();
        }
    });

    QObject::connect(win, &QWindow::visibleChanged, win, [](bool) {
        update_tray_window_actions();
    });
}

inline QWindow* find_main_window() {
    QPointer<QWindow>& win = main_window_ref();
    if (win && win->objectName() == QStringLiteral("librelinkupMainWindow")) {
        ensure_window_hooks(win.data());
        return win.data();
    }
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w && w->objectName() == QStringLiteral("librelinkupMainWindow")) {
            win = w;
            ensure_window_hooks(w);
            return w;
        }
    }
    return nullptr;
}

inline void clamp_window_to_screen(QWindow* win) {
    if (!win) {
        return;
    }
    if (win->objectName() == QStringLiteral("librelinkupMainWindow")
        && !MainWindowPlacementState::instance().hasSavedPosition()) {
        MainWindowPlacementState::instance().applyToWindow(win, true);
        return;
    }
    QScreen* sc = win->screen();
    if (!sc) {
        sc = QGuiApplication::screenAt(win->geometry().center());
    }
    if (!sc) {
        sc = QGuiApplication::primaryScreen();
    }
    if (!sc) {
        return;
    }
    const QRect avail = sc->availableGeometry().isValid() ? sc->availableGeometry() : sc->geometry();
    if (!avail.isValid()) {
        return;
    }
    const int w = std::min(win->width(), avail.width());
    const int h = std::min(win->height(), avail.height());
    const int minX = avail.left();
    const int maxX = std::max(avail.left(), avail.right() - w + 1);
    const int minY = avail.top();
    const int maxY = std::max(avail.top(), avail.bottom() - h + 1);
    const int clampedX = std::clamp(win->x(), minX, maxX);
    const int clampedY = std::clamp(win->y(), minY, maxY);
    if (win->width() != w || win->height() != h) {
        win->resize(w, h);
    }
    if (win->x() != clampedX || win->y() != clampedY) {
        win->setPosition(clampedX, clampedY);
    }
}

inline void restore_main_window_placement(
    const QString& screen_name,
    int x,
    int y,
    int width,
    int height
) {
    auto& st = MainWindowPlacementState::instance();
    st.screenName = screen_name.trimmed();
    st.relX = x;
    st.relY = y;
    st.width = width > 0 ? width : 980;
    st.height = height > 0 ? height : 680;
    st.hasPlacement = true;

    if (QWindow* win = find_main_window()) {
        st.applyToWindow(win, false);
    }
}

inline void record_main_window_placement(QWindow* win, bool persistToSettings) {
    if (!win) {
        return;
    }
    auto& st = MainWindowPlacementState::instance();
    if (st.applying) {
        return;
    }
    QScreen* sc = QGuiApplication::screenAt(win->geometry().center());
    if (!sc) {
        sc = win->screen() ? win->screen() : QGuiApplication::primaryScreen();
    }
    if (!sc) {
        return;
    }
    const QRect scGeom = sc->geometry();
    const QRect avail = sc->availableGeometry().isValid() ? sc->availableGeometry() : scGeom;
    const int minW = std::max(780, win->minimumWidth());
    const int minH = std::max(540, win->minimumHeight());
    const int w = std::clamp(win->width(), minW, std::max(minW, avail.width()));
    const int h = std::clamp(win->height(), minH, std::max(minH, avail.height()));
    const int minX = avail.left();
    const int maxX = std::max(avail.left(), avail.right() - w + 1);
    const int minY = avail.top();
    const int maxY = std::max(avail.top(), avail.bottom() - h + 1);
    const int clampedX = std::clamp(win->x(), minX, maxX);
    const int clampedY = std::clamp(win->y(), minY, maxY);
    const int relX = std::max(0, clampedX - scGeom.x());
    const int relY = std::max(0, clampedY - scGeom.y());

    st.screenName = sc->name();
    st.relX = relX;
    st.relY = relY;
    st.width = w;
    st.height = h;
    st.hasPlacement = true;

    if (persistToSettings) {
        if (QObject* settingsCtrl = win->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "saveMainWindowPlacement",
                Qt::DirectConnection,
                Q_ARG(QString, sc->name()),
                Q_ARG(int, relX),
                Q_ARG(int, relY),
                Q_ARG(int, w),
                Q_ARG(int, h)
            );
        }
    }
}

struct OverlayPlacementState {
    QString screenName;
    int relX = -1;
    int relY = -1;
    int width = 276;
    int height = 88;
    bool hasPlacement = false;
    bool onAllDesktops = true;
    bool dragging = false;
    QPoint dragStartCursor;
    QPoint dragStartWinPos;

    static OverlayPlacementState& instance() {
        static OverlayPlacementState st;
        return st;
    }

    void applyToWindow(QWindow* ovWin) const {
        if (!ovWin || dragging) {
            return;
        }
        QScreen* targetScreen = nullptr;
        const QString cleanName = screenName.trimmed();
        if (!cleanName.isEmpty()) {
            for (QScreen* sc : QGuiApplication::screens()) {
                if (sc && sc->name().compare(cleanName, Qt::CaseInsensitive) == 0) {
                    targetScreen = sc;
                    break;
                }
            }
        }
        if (!targetScreen) {
            targetScreen = QGuiApplication::primaryScreen()
                ? QGuiApplication::primaryScreen()
                : ovWin->screen();
        }
        if (!targetScreen) {
            ovWin->setProperty("placementReady", true);
            return;
        }

        const QRect scGeom = targetScreen->geometry();
        const QRect avail = targetScreen->availableGeometry().isValid()
            ? targetScreen->availableGeometry()
            : scGeom;

        const int w = std::clamp(width, 240, std::max(240, std::min(600, avail.width())));
        const int h = std::clamp(height, 80, std::max(80, std::min(260, avail.height())));

        int globalX = avail.right() - w - 24;
        int globalY = avail.top() + 36;
        if (relX >= 0 && relY >= 0) {
            globalX = scGeom.x() + relX;
            globalY = scGeom.y() + relY;
        }

        const int minX = avail.left();
        const int maxX = std::max(avail.left(), avail.right() - w + 1);
        const int minY = avail.top();
        const int maxY = std::max(avail.top(), avail.bottom() - h + 1);
        const int clampedX = std::clamp(globalX, minX, maxX);
        const int clampedY = std::clamp(globalY, minY, maxY);

        if (ovWin->screen() != targetScreen) {
            ovWin->setScreen(targetScreen);
        }
        if (ovWin->width() != w || ovWin->height() != h) {
            ovWin->resize(w, h);
        }
        if (ovWin->x() != clampedX || ovWin->y() != clampedY) {
            ovWin->setPosition(clampedX, clampedY);
        }
        ovWin->setProperty("placementReady", true);
    }
};

inline void ensure_overlay_hooks(QWindow* ovWin) {
    if (!ovWin || ovWin->property("_llu_overlay_hooked").toBool()) {
        return;
    }
    ovWin->setProperty("_llu_overlay_hooked", true);
    if (ovWin->transientParent() != nullptr) {
        ovWin->setTransientParent(nullptr);
    }
    QObject::connect(ovWin, &QWindow::transientParentChanged, ovWin, [ovWin](QWindow* parent) {
        if (parent != nullptr) {
            ovWin->setTransientParent(nullptr);
        }
    });
    QObject::connect(ovWin, &QWindow::visibleChanged, ovWin, [ovWin](bool vis) {
        if (vis) {
            if (OverlayPlacementState::instance().hasPlacement) {
                OverlayPlacementState::instance().applyToWindow(ovWin);
                QTimer::singleShot(30, ovWin, [ovWin]() {
                    if (ovWin && ovWin->isVisible()) {
                        OverlayPlacementState::instance().applyToWindow(ovWin);
                    }
                });
                QTimer::singleShot(120, ovWin, [ovWin]() {
                    if (ovWin && ovWin->isVisible()) {
                        OverlayPlacementState::instance().applyToWindow(ovWin);
                    }
                });
            } else {
                clamp_window_to_screen(ovWin);
            }
        }
        update_tray_window_actions();
    });
}

inline QWindow* find_overlay_window() {
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w && w->objectName() == QStringLiteral("librelinkupOverlayWindow")) {
            ensure_overlay_hooks(w);
            return w;
        }
    }
    return nullptr;
}

inline void restore_overlay_placement(
    const QString& screen_name,
    int x,
    int y,
    int width,
    int height
) {
    auto& st = OverlayPlacementState::instance();
    st.screenName = screen_name.trimmed();
    st.relX = x;
    st.relY = y;
    st.width = width;
    st.height = height;
    st.hasPlacement = true;

    if (QWindow* ovWin = find_overlay_window()) {
        st.applyToWindow(ovWin);
    }
}

inline void apply_overlay_all_desktops(bool onAllDesktops) {
    auto& st = OverlayPlacementState::instance();
    st.onAllDesktops = onAllDesktops;

    if (QWindow* ovWin = find_overlay_window()) {
        const Qt::WindowFlags desiredFlags = Qt::Tool
            | Qt::FramelessWindowHint
            | Qt::WindowStaysOnTopHint
            | (onAllDesktops ? Qt::BypassWindowManagerHint : Qt::WindowFlags());
        if (ovWin->flags() != desiredFlags) {
            const bool wasVis = ovWin->isVisible();
            ovWin->setFlags(desiredFlags);
            if (wasVis) {
                ovWin->show();
                st.applyToWindow(ovWin);
            }
        }
    }

    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }

    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    const QString runtimeDir = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation).isEmpty()
        ? QDir::tempPath()
        : QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
    // Unique, per-invocation script name and O_CREAT|O_EXCL: never follow a pre-existing
    // path (which could be a symlink planted by another process in the runtime dir), and
    // never reuse a file whose permissions we have not set ourselves.
    const QString scriptPath = runtimeDir
        + QStringLiteral("/librelinkup-kwin-%1.js").arg(QUuid::createUuid().toString(QUuid::Id128));
    QFile f(scriptPath);
    if (f.open(QIODevice::NewOnly | QIODevice::WriteOnly | QIODevice::Text)) {
        f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        const QString js = QStringLiteral(
            "var list = workspace.stackingOrder || workspace.windowList();\n"
            "if (list) {\n"
            "  for (var i = 0; i < list.length; ++i) {\n"
            "    var w = list[i];\n"
            "    var cap = w && w.caption ? w.caption.toString() : '';\n"
            "    if (cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0) {\n"
            "      w.onAllDesktops = %1;\n"
            "      w.keepAbove = true;\n"
            "      w.skipTaskbar = true;\n"
            "      w.skipPager = true;\n"
            "    }\n"
            "  }\n"
            "}\n"
        ).arg(onAllDesktops ? QStringLiteral("true") : QStringLiteral("false"));
        const bool wrote = f.write(js.toUtf8()) >= 0;
        f.close();
        if (!wrote) {
            QFile::remove(scriptPath);
            return;
        }

        const QString pluginName = QStringLiteral("librelinkup_overlay_desktop_oneshot");
        QDBusMessage unloadMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("unloadScript")
        );
        unloadMsg << pluginName;
        bus.call(unloadMsg, QDBus::NoBlock);

        QDBusMessage loadMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("loadScript")
        );
        loadMsg << scriptPath << pluginName;
        QDBusReply<int> loadReply = bus.call(loadMsg);
        if (loadReply.isValid() && loadReply.value() >= 0) {
            const QString scriptObj = QStringLiteral("/Scripting/Script%1").arg(loadReply.value());
            QDBusMessage runMsg = QDBusMessage::createMethodCall(
                QStringLiteral("org.kde.KWin"),
                scriptObj,
                QStringLiteral("org.kde.kwin.Script"),
                QStringLiteral("run")
            );
            bus.call(runMsg, QDBus::NoBlock);
            QTimer::singleShot(80, [pluginName, scriptPath]() {
                QFile::remove(scriptPath);
                QDBusMessage cleanupMsg = QDBusMessage::createMethodCall(
                    QStringLiteral("org.kde.KWin"),
                    QStringLiteral("/Scripting"),
                    QStringLiteral("org.kde.kwin.Scripting"),
                    QStringLiteral("unloadScript")
                );
                cleanupMsg << pluginName;
                QDBusConnection::sessionBus().call(cleanupMsg, QDBus::NoBlock);
            });
        }
    }
}

inline void begin_overlay_drag() {
    QWindow* ovWin = find_overlay_window();
    if (!ovWin) {
        return;
    }
    auto& st = OverlayPlacementState::instance();
    st.dragging = true;
    st.dragStartCursor = QCursor::pos();
    st.dragStartWinPos = ovWin->position();
}

inline void update_overlay_drag() {
    QWindow* ovWin = find_overlay_window();
    auto& st = OverlayPlacementState::instance();
    if (!ovWin || !st.dragging) {
        return;
    }
    const QPoint nextPos = st.dragStartWinPos + (QCursor::pos() - st.dragStartCursor);
    ovWin->setPosition(nextPos);
}

inline void end_overlay_drag(QObject* controller) {
    auto& st = OverlayPlacementState::instance();
    st.dragging = false;
    QWindow* ovWin = find_overlay_window();
    if (!ovWin) {
        return;
    }

    QScreen* sc = QGuiApplication::screenAt(ovWin->geometry().center());
    if (!sc) {
        sc = QGuiApplication::screenAt(QCursor::pos());
    }
    if (!sc) {
        sc = ovWin->screen() ? ovWin->screen() : QGuiApplication::primaryScreen();
    }
    if (!sc) {
        return;
    }

    const QRect scGeom = sc->geometry();
    const QRect avail = sc->availableGeometry().isValid() ? sc->availableGeometry() : scGeom;
    const int w = std::min(ovWin->width(), avail.width());
    const int h = std::min(ovWin->height(), avail.height());
    const int minX = avail.left();
    const int maxX = std::max(avail.left(), avail.right() - w + 1);
    const int minY = avail.top();
    const int maxY = std::max(avail.top(), avail.bottom() - h + 1);
    const int clampedX = std::clamp(ovWin->x(), minX, maxX);
    const int clampedY = std::clamp(ovWin->y(), minY, maxY);

    if (ovWin->screen() != sc) {
        ovWin->setScreen(sc);
    }
    if (ovWin->width() != w || ovWin->height() != h) {
        ovWin->resize(w, h);
    }
    if (ovWin->x() != clampedX || ovWin->y() != clampedY) {
        ovWin->setPosition(clampedX, clampedY);
    }

    const int relX = clampedX - scGeom.x();
    const int relY = clampedY - scGeom.y();

    st.screenName = sc->name();
    st.relX = relX;
    st.relY = relY;
    st.width = w;
    st.height = h;
    st.hasPlacement = true;

    if (controller) {
        QMetaObject::invokeMethod(
            controller,
            "saveOverlayPlacement",
            Qt::DirectConnection,
            Q_ARG(QString, sc->name()),
            Q_ARG(int, relX),
            Q_ARG(int, relY),
            Q_ARG(int, w),
            Q_ARG(int, h)
        );
    }
}

inline QWindow* find_log_viewer_window() {
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w && w->objectName() == QStringLiteral("librelinkupLogViewerWindow")) {
            return w;
        }
    }
    return nullptr;
}

inline QWindow* find_about_window() {
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w && w->objectName() == QStringLiteral("librelinkupAboutWindow")) {
            return w;
        }
    }
    return nullptr;
}

inline bool is_main_window_shown() {
    QWindow* win = find_main_window();
    if (!win) {
        return false;
    }
    return win->isVisible()
        && !(win->windowStates() & Qt::WindowMinimized)
        && win->visibility() != QWindow::Minimized
        && win->visibility() != QWindow::Hidden;
}

inline void show_main_window() {
    if (QWindow* win = find_main_window()) {
        if (win->windowStates() & Qt::WindowMinimized) {
            win->setWindowStates(win->windowStates() & ~Qt::WindowMinimized);
        }
        MainWindowPlacementState::instance().applyToWindow(win, true);
        win->showNormal();
        win->show();
        MainWindowPlacementState::instance().applyToWindow(win, true);
        win->raise();
        win->requestActivate();
        QMetaObject::invokeMethod(win, "recheckHeaderWidth", Qt::QueuedConnection);
    }
    update_tray_window_actions();
}

inline void hide_main_window_to_tray() {
    if (QWindow* win = find_main_window()) {
        if (win->isVisible() && !(win->windowStates() & Qt::WindowMinimized)) {
            const bool persist = qEnvironmentVariable("QT_QPA_PLATFORM") != QStringLiteral("offscreen");
            record_main_window_placement(win, persist);
        }
        win->hide();
        if (win->windowStates() & Qt::WindowMinimized) {
            win->setWindowStates(win->windowStates() & ~Qt::WindowMinimized);
        }
    }
    update_tray_window_actions();
}

inline void show_log_viewer_window() {
    show_main_window();
    if (QWindow* mainWin = find_main_window()) {
        QMetaObject::invokeMethod(mainWin, "openLogViewer", Qt::QueuedConnection);
    }
}

inline void show_about_window() {
    show_main_window();
    if (QWindow* mainWin = find_main_window()) {
        QMetaObject::invokeMethod(mainWin, "openAboutDialog", Qt::QueuedConnection);
    }
}

/// Application-wide high-resolution wheel scroll filter for Qt Quick (`QQuickFlickable` /
/// `ListView` / `QQC2.ScrollView`).
///
/// High-resolution scroll devices (such as Ploopy Trackball / Ploopy Knob sending
/// `REL_WHEEL_HI_RES` v120 micro-steps where `angleDelta.y()` is small e.g. `±2`..`±15`
/// with `Qt::NoScrollPhase`) get swallowed by Qt Quick's default `QQuickFlickable`
/// velocity deadzone and trigger expensive hover re-evaluations on every micro-step.
///
/// This filter:
/// 1. Linearly converts `angleDelta()` (`dyPx = angle.y() * 0.50`, i.e. `60 px` per `120`
///    v120 units, or `pixelDelta()` fallback) with zero deadzone.
/// 2. Updates `contentY` / `contentX` directly on the deepest scrollable `QQuickFlickable`
///    under the cursor (locking onto that flickable during a continuous 350 ms scroll gesture).
/// 3. Suppresses scene-graph `HoverEnter`, `HoverMove`, and `ToolTip` events during active
///    wheel scrolling so 1000 Hz Ploopy micro-steps scroll at 60+ FPS without lag or tooltips.
class LibreLinkUpWheelScrollFilter : public QObject {
public:
    explicit LibreLinkUpWheelScrollFilter(QObject* parent = nullptr)
        : QObject(parent)
    {
        m_hoverRestoreTimer.setSingleShot(true);
        m_hoverRestoreTimer.setInterval(400);
        QObject::connect(&m_hoverRestoreTimer, &QTimer::timeout, this, [this]() {
            restoreHoverAfterScroll();
        });
    }

protected:
    bool eventFilter(QObject* watched, QEvent* event) override {
        if (!event) {
            return QObject::eventFilter(watched, event);
        }

        const QEvent::Type et = event->type();

        // While actively wheel-scrolling, suppress HoverEnter/HoverMove/ToolTip events on scene
        // items so QQuickWindow does not re-evaluate hover states or pop up tooltips on every
        // high-frequency Ploopy micro-step.
        if (m_wheelScrollingActive) {
            if (et == QEvent::HoverEnter || et == QEvent::HoverMove || et == QEvent::ToolTip) {
                return true;
            }
            if (et == QEvent::MouseMove) {
                auto* me = static_cast<QMouseEvent*>(event);
                const QPointF delta = me->globalPosition() - m_lastWheelGlobalPos;
                if (std::hypot(delta.x(), delta.y()) <= 3.0) {
                    return true;
                }
                restoreHoverAfterScroll();
            }
        }

        if (et == QEvent::KeyPress) {
            auto* quickWin = qobject_cast<QQuickWindow*>(watched);
            if (quickWin && quickWin->objectName() == QStringLiteral("librelinkupMainWindow")) {
                auto* ke = static_cast<QKeyEvent*>(event);
                const Qt::KeyboardModifiers mods = ke->modifiers();
                const int key = ke->key();
                QObject* settingsCtrl = quickWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"));
                QObject* appCtrl = quickWin->findChild<QObject*>(QStringLiteral("librelinkupAppController"));

                if ((mods & Qt::ControlModifier) != 0 && (mods & Qt::AltModifier) == 0) {
                    if (key == Qt::Key_Plus || key == Qt::Key_Equal) {
                        if (settingsCtrl) {
                            QMetaObject::invokeMethod(settingsCtrl, "zoomIn", Qt::DirectConnection);
                            ke->accept();
                            return true;
                        }
                    } else if (key == Qt::Key_Minus || key == Qt::Key_Underscore) {
                        if (settingsCtrl) {
                            QMetaObject::invokeMethod(settingsCtrl, "zoomOut", Qt::DirectConnection);
                            ke->accept();
                            return true;
                        }
                    } else if (key == Qt::Key_0 || key == Qt::Key_ParenRight) {
                        if (settingsCtrl) {
                            QMetaObject::invokeMethod(settingsCtrl, "resetZoom", Qt::DirectConnection);
                            ke->accept();
                            return true;
                        }
                    } else if (key == Qt::Key_R) {
                        if (appCtrl) {
                            QMetaObject::invokeMethod(appCtrl, "refreshGlucoseNow", Qt::DirectConnection);
                            ke->accept();
                            return true;
                        }
                    }
                } else if (mods == Qt::NoModifier && key == Qt::Key_F5) {
                    if (appCtrl) {
                        QMetaObject::invokeMethod(appCtrl, "refreshGlucoseNow", Qt::DirectConnection);
                        ke->accept();
                        return true;
                    }
                } else if ((mods & Qt::AltModifier) != 0 && (mods & (Qt::ControlModifier | Qt::MetaModifier)) == 0) {
                    QString targetTab;
                    switch (key) {
                        case Qt::Key_1: targetTab = QStringLiteral("dashboard"); break;
                        case Qt::Key_2: targetTab = QStringLiteral("medications"); break;
                        case Qt::Key_3: targetTab = QStringLiteral("general"); break;
                        case Qt::Key_4: targetTab = QStringLiteral("alerts"); break;
                        case Qt::Key_5: targetTab = QStringLiteral("account"); break;
                        case Qt::Key_6: targetTab = QStringLiteral("logs"); break;
                        case Qt::Key_7: targetTab = QStringLiteral("about"); break;
                        default: break;
                    }
                    if (!targetTab.isEmpty()) {
                        quickWin->setProperty("activeTab", targetTab);
                        ke->accept();
                        return true;
                    }
                }
            }
            return QObject::eventFilter(watched, event);
        }

        if (et != QEvent::Wheel) {
            return QObject::eventFilter(watched, event);
        }

        auto* quickWin = qobject_cast<QQuickWindow*>(watched);
        if (!quickWin || !quickWin->contentItem()) {
            return QObject::eventFilter(watched, event);
        }

        // Do not intercept wheel events on the floating desktop overlay window, which uses
        // the mouse wheel to adjust overlay background opacity.
        if (quickWin->objectName() == QStringLiteral("librelinkupOverlayWindow")) {
            return QObject::eventFilter(watched, event);
        }

        auto* we = static_cast<QWheelEvent*>(event);
        if ((we->modifiers() & Qt::ControlModifier) != 0) {
            if (quickWin->objectName() == QStringLiteral("librelinkupMainWindow")) {
                if (QObject* settingsCtrl = quickWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
                    const int dy = !we->angleDelta().isNull() ? we->angleDelta().y() : we->pixelDelta().y();
                    if (dy > 0) {
                        QMetaObject::invokeMethod(settingsCtrl, "zoomIn", Qt::DirectConnection);
                        we->accept();
                        return true;
                    } else if (dy < 0) {
                        QMetaObject::invokeMethod(settingsCtrl, "zoomOut", Qt::DirectConnection);
                        we->accept();
                        return true;
                    }
                }
            }
            return QObject::eventFilter(watched, event);
        }

        const QPoint angle = we->angleDelta();
        const QPoint pixel = we->pixelDelta();
        qreal dyPx = 0.0;
        qreal dxPx = 0.0;
        if (!angle.isNull()) {
            // Standard wheel notch = 120 units -> 60 px; Ploopy hi-res micro-step = 2..15 units -> 1..7.5 px.
            dyPx = static_cast<qreal>(angle.y()) * 0.50;
            dxPx = static_cast<qreal>(angle.x()) * 0.50;
        } else if (!pixel.isNull()) {
            dyPx = static_cast<qreal>(pixel.y());
            dxPx = static_cast<qreal>(pixel.x());
        }
        if (qFuzzyIsNull(dyPx) && qFuzzyIsNull(dxPx)) {
            return QObject::eventFilter(watched, event);
        }

        beginWheelScrollHoverSuppression(quickWin, we);

        const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
        const QPointF cursorDelta = we->position() - m_lastWheelScenePos;
        if (m_lockedFlickable
            && (nowMs - m_lastWheelLockMs) <= 350
            && std::hypot(cursorDelta.x(), cursorDelta.y()) <= 4.0
            && isItemEffectivelyVisible(m_lockedFlickable)) {
            if (scrollFlickableDirect(m_lockedFlickable, dxPx, dyPx)) {
                m_lastWheelLockMs = nowMs;
                m_lastWheelScenePos = we->position();
                we->accept();
                return true;
            }
        }

        QQuickItem* hitFlickable = nullptr;
        if (findAndScrollFlickableAt(quickWin->contentItem(), we->position(), dxPx, dyPx, &hitFlickable)) {
            m_lockedFlickable = hitFlickable;
            m_lastWheelLockMs = nowMs;
            m_lastWheelScenePos = we->position();
            we->accept();
            return true;
        }

        return QObject::eventFilter(watched, event);
    }

private:
    QPointer<QQuickWindow> m_scrollWindow;
    QPointer<QQuickItem> m_lockedFlickable;
    QPointF m_lastWheelScenePos;
    QPointF m_lastWheelGlobalPos;
    Qt::KeyboardModifiers m_lastWheelModifiers = Qt::NoModifier;
    qint64 m_lastWheelLockMs = 0;
    bool m_wheelScrollingActive = false;
    bool m_sendingSyntheticHover = false;
    QTimer m_hoverRestoreTimer;

    void beginWheelScrollHoverSuppression(QQuickWindow* win, QWheelEvent* we) {
        m_scrollWindow = win;
        m_lastWheelScenePos = we->position();
        m_lastWheelGlobalPos = we->globalPosition();
        m_lastWheelModifiers = we->modifiers();
        if (!m_wheelScrollingActive && !m_sendingSyntheticHover) {
            m_sendingSyntheticHover = true;
            QHoverEvent leaveEv(QEvent::HoverLeave, QPointF(-10000.0, -10000.0), m_lastWheelGlobalPos, m_lastWheelScenePos, m_lastWheelModifiers);
            QCoreApplication::sendEvent(win, &leaveEv);
            m_sendingSyntheticHover = false;
            m_wheelScrollingActive = true;
        }
        m_hoverRestoreTimer.start();
    }

    void restoreHoverAfterScroll() {
        if (!m_wheelScrollingActive) {
            return;
        }
        m_hoverRestoreTimer.stop();
        m_wheelScrollingActive = false;
        m_lockedFlickable = nullptr;
        if (m_scrollWindow && m_scrollWindow->isVisible() && !m_sendingSyntheticHover) {
            m_sendingSyntheticHover = true;
            const QPoint globalPos = QCursor::pos();
            const QPointF scenePos = m_scrollWindow->mapFromGlobal(globalPos);
            QHoverEvent moveEv(QEvent::HoverMove, scenePos, QPointF(globalPos), scenePos, m_lastWheelModifiers);
            QCoreApplication::sendEvent(m_scrollWindow.data(), &moveEv);
            m_sendingSyntheticHover = false;
        }
    }

    static bool isItemEffectivelyVisible(QQuickItem* item) {
        for (QQuickItem* cur = item; cur != nullptr; cur = cur->parentItem()) {
            if (!cur->isVisible() || !cur->isEnabled() || cur->opacity() <= 0.01) {
                return false;
            }
        }
        return true;
    }

    static bool isInteractiveWheelControl(QQuickItem* item) {
        if (!item) {
            return false;
        }
        return item->inherits("QQuickSlider")
            || item->inherits("QQuickSpinBox")
            || item->inherits("QQuickDial")
            || item->inherits("QQuickTumbler")
            || item->inherits("QQuickComboBox");
    }

    static bool scrollFlickableDirect(QQuickItem* flick, qreal dxPx, qreal dyPx) {
        if (!flick || !flick->inherits("QQuickFlickable")) {
            return false;
        }
        const bool interactive = flick->property("interactive").toBool();
        if (!interactive && !flick->inherits("QQuickItemView")) {
            return false;
        }

        const qreal viewH = flick->height();
        const qreal contentH = flick->property("contentHeight").toReal();
        const qreal topMargin = flick->property("topMargin").toReal();
        const qreal bottomMargin = flick->property("bottomMargin").toReal();
        const qreal originY = flick->property("originY").toReal();
        const qreal curY = flick->property("contentY").toReal();

        const qreal viewW = flick->width();
        const qreal contentW = flick->property("contentWidth").toReal();
        const qreal leftMargin = flick->property("leftMargin").toReal();
        const qreal rightMargin = flick->property("rightMargin").toReal();
        const qreal originX = flick->property("originX").toReal();
        const qreal curX = flick->property("contentX").toReal();

        bool scrolled = false;

        if (!qFuzzyIsNull(dyPx) && viewH > 0.0 && (contentH + topMargin + bottomMargin) > viewH + 0.5) {
            const qreal minY = originY - topMargin;
            const qreal maxY = std::max(minY, originY + contentH + bottomMargin - viewH);
            const qreal nextY = std::clamp(curY - dyPx, minY, maxY);
            if (!qFuzzyCompare(nextY + 1.0, curY + 1.0)) {
                QMetaObject::invokeMethod(flick, "cancelFlick", Qt::DirectConnection);
                flick->setProperty("contentY", nextY);
                scrolled = true;
            } else if ((dyPx > 0.0 && curY <= minY + 0.5) || (dyPx < 0.0 && curY >= maxY - 0.5)) {
                // Already at the boundary of this flickable; consume if it is the primary view.
                scrolled = true;
            }
        }

        if (!scrolled && !qFuzzyIsNull(dxPx) && viewW > 0.0 && (contentW + leftMargin + rightMargin) > viewW + 0.5) {
            const qreal minX = originX - leftMargin;
            const qreal maxX = std::max(minX, originX + contentW + rightMargin - viewW);
            const qreal nextX = std::clamp(curX - dxPx, minX, maxX);
            if (!qFuzzyCompare(nextX + 1.0, curX + 1.0)) {
                QMetaObject::invokeMethod(flick, "cancelFlick", Qt::DirectConnection);
                flick->setProperty("contentX", nextX);
                scrolled = true;
            } else if ((dxPx > 0.0 && curX <= minX + 0.5) || (dxPx < 0.0 && curX >= maxX - 0.5)) {
                scrolled = true;
            }
        }

        return scrolled;
    }

    static bool findAndScrollFlickableAt(
        QQuickItem* item,
        const QPointF& scenePos,
        qreal dxPx,
        qreal dyPx,
        QQuickItem** outFlickable = nullptr
    ) {
        if (!item || !item->isVisible() || !item->isEnabled() || item->opacity() <= 0.01) {
            return false;
        }

        const QPointF localPos = item->mapFromScene(scenePos);
        const bool insideSelf = item->contains(localPos);

        if (item->clip() && !insideSelf) {
            return false;
        }

        // If hovering directly over an interactive wheel control (Slider, SpinBox, ComboBox),
        // let that control or the outer scroll view handle it appropriately.
        if (insideSelf && isInteractiveWheelControl(item)) {
            return false;
        }

        // Check children in reverse paint/z order (topmost visual child first).
        QList<QQuickItem*> children = item->childItems();
        std::stable_sort(children.begin(), children.end(), [](const QQuickItem* a, const QQuickItem* b) {
            return a->z() < b->z();
        });
        for (int i = children.size() - 1; i >= 0; --i) {
            if (findAndScrollFlickableAt(children.at(i), scenePos, dxPx, dyPx, outFlickable)) {
                return true;
            }
        }

        if (!insideSelf) {
            return false;
        }

        // If the cursor is directly over a HighContrastScrollBar or QQuickScrollBar,
        // scroll its attached Flickable (either parentItem() for ListView or
        // parentItem()->property("contentItem") for QQC2.ScrollView).
        if (item->objectName().contains(QStringLiteral("ScrollBar"), Qt::CaseInsensitive)
            || item->inherits("QQuickScrollBar")) {
            if (QQuickItem* p = item->parentItem()) {
                if (scrollFlickableDirect(p, dxPx, dyPx)) {
                    if (outFlickable) {
                        *outFlickable = p;
                    }
                    return true;
                }
                if (QQuickItem* ci = qobject_cast<QQuickItem*>(p->property("contentItem").value<QObject*>())) {
                    if (scrollFlickableDirect(ci, dxPx, dyPx)) {
                        if (outFlickable) {
                            *outFlickable = ci;
                        }
                        return true;
                    }
                }
            }
        }

        // If this item is a QQC2.ScrollView, check its internal contentItem QQuickFlickable.
        if (item->inherits("QQuickScrollView")) {
            if (QQuickItem* ci = qobject_cast<QQuickItem*>(item->property("contentItem").value<QObject*>())) {
                if (scrollFlickableDirect(ci, dxPx, dyPx)) {
                    if (outFlickable) {
                        *outFlickable = ci;
                    }
                    return true;
                }
            }
        }

        if (scrollFlickableDirect(item, dxPx, dyPx)) {
            if (outFlickable) {
                *outFlickable = item;
            }
            return true;
        }

        return false;
    }
};

inline void install_wheel_scroll_filter() {
    static LibreLinkUpWheelScrollFilter* s_filter = nullptr;
    if (!s_filter && QCoreApplication::instance()) {
        s_filter = new LibreLinkUpWheelScrollFilter(QCoreApplication::instance());
        QCoreApplication::instance()->installEventFilter(s_filter);
    }
}

} // namespace librelinkup

inline void librelinkup_show_main_window() {
    librelinkup::show_main_window();
}

inline void librelinkup_hide_main_window() {
    librelinkup::hide_main_window_to_tray();
}

inline void librelinkup_restore_overlay_placement(
    const QString& screen_name,
    std::int32_t x,
    std::int32_t y,
    std::int32_t width,
    std::int32_t height
) {
    librelinkup::restore_overlay_placement(screen_name, x, y, width, height);
    const QString scCopy = screen_name;
    for (int delayMs : {0, 40, 140}) {
        QTimer::singleShot(delayMs, [scCopy, x, y, width, height]() {
            librelinkup::restore_overlay_placement(scCopy, x, y, width, height);
            librelinkup::update_tray_window_actions();
        });
    }
}

inline void librelinkup_restore_main_window_placement(
    const QString& screen_name,
    std::int32_t x,
    std::int32_t y,
    std::int32_t width,
    std::int32_t height
) {
    librelinkup::restore_main_window_placement(screen_name, x, y, width, height);
}

inline void librelinkup_apply_overlay_all_desktops(bool on_all_desktops) {
    librelinkup::apply_overlay_all_desktops(on_all_desktops);
}

inline void librelinkup_begin_overlay_drag() {
    librelinkup::begin_overlay_drag();
}

inline void librelinkup_update_overlay_drag() {
    librelinkup::update_overlay_drag();
}

template <typename ControllerT>
inline void librelinkup_end_overlay_drag(ControllerT& controller) {
    librelinkup::end_overlay_drag(&controller);
}

inline void librelinkup_trigger_window_alert(bool bring_to_front, bool flash_window) {
    if (QWindow* win = librelinkup::find_main_window()) {
        if (bring_to_front) {
            librelinkup::show_main_window();
        }
        if (flash_window) {
            win->alert(0);
        }
    }
}
