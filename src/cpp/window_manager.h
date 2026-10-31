#pragma once

#include <QCursor>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QFileDevice>
#include <QGuiApplication>
#include <QMetaObject>
#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QRect>
#include <QScreen>
#include <QUuid>
#include <QStandardPaths>
#include <QString>
#include <QTimer>
#include <QWindow>
#include <algorithm>
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
