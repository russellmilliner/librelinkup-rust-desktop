#pragma once

#include <QCoreApplication>
#include <QCursor>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusReply>
#include <QDBusServiceWatcher>
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
inline bool is_wayland_platform() {
    return QGuiApplication::platformName().startsWith(QStringLiteral("wayland"), Qt::CaseInsensitive);
}
inline void ensure_kwin_dbus_bridge();
inline void run_kwin_oneshot_script(const QString& jsBody);
inline void sync_kwin_overlay_window(int globalX, int globalY, int w, int h, bool onAllDesktops);
inline void sync_kwin_main_window(int globalX, int globalY, int w, int h);
inline void query_kwin_overlay_geometry();
inline void query_kwin_main_window_geometry();

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
        if (!is_wayland_platform()) {
            if (win->x() != clampedX || win->y() != clampedY) {
                win->setPosition(clampedX, clampedY);
            }
        } else if (win->isVisible()) {
            sync_kwin_main_window(clampedX, clampedY, w, h);
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
    if (is_wayland_platform()) {
        query_kwin_main_window_geometry();
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
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relX)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relY)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(w)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(h))
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
    bool systemMoveStarted = false;
    QPoint dragStartCursor;
    QPoint dragStartWinPos;
    mutable int lastKWinX = -999999;
    mutable int lastKWinY = -999999;
    mutable int lastKWinW = 0;
    mutable int lastKWinH = 0;
    mutable bool lastKWinAllDesktops = true;
    mutable bool lastKWinSynced = false;

    static OverlayPlacementState& instance() {
        static OverlayPlacementState st;
        return st;
    }

    void invalidateKWinSync() const {
        lastKWinSynced = false;
    }

    bool computeGlobalGeometry(
        QWindow* ovWin,
        QScreen*& outScreen,
        int& outX,
        int& outY,
        int& outW,
        int& outH
    ) const {
        outScreen = nullptr;
        const QString cleanName = screenName.trimmed();
        if (!cleanName.isEmpty()) {
            for (QScreen* sc : QGuiApplication::screens()) {
                if (sc && sc->name().compare(cleanName, Qt::CaseInsensitive) == 0) {
                    outScreen = sc;
                    break;
                }
            }
        }
        if (!outScreen) {
            outScreen = QGuiApplication::primaryScreen()
                ? QGuiApplication::primaryScreen()
                : (ovWin ? ovWin->screen() : nullptr);
        }
        if (!outScreen) {
            return false;
        }

        const QRect scGeom = outScreen->geometry();
        const QRect avail = outScreen->availableGeometry().isValid()
            ? outScreen->availableGeometry()
            : scGeom;

        outW = std::clamp(width, 240, std::max(240, std::min(600, avail.width())));
        outH = std::clamp(height, 80, std::max(80, std::min(260, avail.height())));

        int globalX = avail.right() - outW - 24;
        int globalY = avail.top() + 36;
        if (relX >= 0 && relY >= 0) {
            globalX = scGeom.x() + relX;
            globalY = scGeom.y() + relY;
        }

        const int minX = avail.left();
        const int maxX = std::max(avail.left(), avail.right() - outW + 1);
        const int minY = avail.top();
        const int maxY = std::max(avail.top(), avail.bottom() - outH + 1);
        outX = std::clamp(globalX, minX, maxX);
        outY = std::clamp(globalY, minY, maxY);
        return true;
    }

    void applyToWindow(QWindow* ovWin, bool forceKWinSync = false) const {
        if (!ovWin || dragging) {
            return;
        }
        QScreen* targetScreen = nullptr;
        int clampedX = 0;
        int clampedY = 0;
        int w = 276;
        int h = 88;
        if (!computeGlobalGeometry(ovWin, targetScreen, clampedX, clampedY, w, h)) {
            ovWin->setProperty("placementReady", true);
            return;
        }

        if ((!is_wayland_platform() || !ovWin->isVisible()) && ovWin->screen() != targetScreen) {
            ovWin->setScreen(targetScreen);
        }
        if (ovWin->width() != w || ovWin->height() != h) {
            ovWin->resize(w, h);
        }
        if (!is_wayland_platform() || !ovWin->isVisible()) {
            if (ovWin->x() != clampedX || ovWin->y() != clampedY) {
                ovWin->setPosition(clampedX, clampedY);
            }
        }
        if (is_wayland_platform() && ovWin->isVisible()) {
            if (forceKWinSync
                || !lastKWinSynced
                || lastKWinX != clampedX
                || lastKWinY != clampedY
                || lastKWinW != w
                || lastKWinH != h
                || lastKWinAllDesktops != onAllDesktops) {
                lastKWinX = clampedX;
                lastKWinY = clampedY;
                lastKWinW = w;
                lastKWinH = h;
                lastKWinAllDesktops = onAllDesktops;
                lastKWinSynced = true;
                sync_kwin_overlay_window(clampedX, clampedY, w, h, onAllDesktops);
            }
        }
        ovWin->setProperty("placementReady", true);
    }
};

inline QString kwin_script_dir() {
    // Prefer ~/.config/librelinkup-kde first because it is guaranteed to reside on the
    // real host filesystem both natively and inside the Flatpak sandbox
    // (--filesystem=xdg-config/librelinkup-kde:create), whereas /run/user/1000/librelinkup-kde
    // inside Flatpak is an isolated container tmpfs if the directory did not already exist
    // on the host prior to bwrap startup.
    const QString hostSharedDir = QDir::homePath() + QStringLiteral("/.config/librelinkup-kde");
    QDir d(hostSharedDir);
    if (d.exists() || d.mkpath(QStringLiteral("."))) {
        return hostSharedDir;
    }
    const QString runtimeBase = QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
    if (!runtimeBase.isEmpty()) {
        const QString runtimeSubdir = runtimeBase + QStringLiteral("/librelinkup-kde");
        QDir rd(runtimeSubdir);
        if (rd.exists() || rd.mkpath(QStringLiteral("."))) {
            QFile::setPermissions(
                runtimeSubdir,
                QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner
            );
            return runtimeSubdir;
        }
    }
    return runtimeBase.isEmpty() ? QDir::tempPath() : runtimeBase;
}

inline int& resident_kwin_script_id_ref() {
    static int s_id = -1;
    return s_id;
}

inline QString& kwin_dbus_service_name_ref() {
    static QString s_service;
    return s_service;
}

inline QString kwin_dbus_interface_name() {
    return QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.KWinBridge");
}

inline QString resident_kwin_sid_path() {
    return kwin_script_dir() + QStringLiteral("/.librelinkup-kwin-resident.sid");
}

inline QString resident_kwin_js_path() {
    return kwin_script_dir() + QStringLiteral("/.librelinkup-kwin-resident.js");
}

struct DesiredKWinSyncState {
    int ovX = 0;
    int ovY = 0;
    int ovW = 0;
    int ovH = 0;
    bool ovAllDesktops = true;
    int mainX = 0;
    int mainY = 0;
    int mainW = 0;
    int mainH = 0;

    static DesiredKWinSyncState& instance() {
        static DesiredKWinSyncState st;
        return st;
    }
};

inline QString format_desired_overlay_sync() {
    const auto& st = DesiredKWinSyncState::instance();
    return QStringLiteral("%1,%2,%3,%4,%5")
        .arg(st.ovX)
        .arg(st.ovY)
        .arg(st.ovW)
        .arg(st.ovH)
        .arg(st.ovAllDesktops ? 1 : 0);
}

inline QString format_desired_main_sync() {
    const auto& st = DesiredKWinSyncState::instance();
    return QStringLiteral("%1,%2,%3,%4")
        .arg(st.mainX)
        .arg(st.mainY)
        .arg(st.mainW)
        .arg(st.mainH);
}

inline void stop_and_unload_resident_script() {
    const QString sidPath = resident_kwin_sid_path();
    const int memId = resident_kwin_script_id_ref();
    int fileId = -1;
    {
        QFile f(sidPath);
        if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
            bool ok = false;
            const int parsed = QString::fromUtf8(f.readAll()).trimmed().toInt(&ok);
            if (ok && parsed >= 0) {
                fileId = parsed;
            }
        }
    }
    resident_kwin_script_id_ref() = -1;
    QFile::remove(sidPath);

    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }

    const QString pluginName = QStringLiteral("librelinkup_kwin_resident_watcher");
    bool isLoaded = true;
    {
        QDBusMessage checkMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("isScriptLoaded")
        );
        checkMsg << pluginName;
        QDBusReply<bool> checkReply = bus.call(checkMsg);
        if (checkReply.isValid()) {
            isLoaded = checkReply.value();
        }
    }

    // Do NOT call `/Scripting/Script<id>.stop()` by numeric ID: in KWin 6, unloading any
    // earlier script leaves sparse `/Scripting/Script<N>` D-Bus object paths so `scripts.size()`
    // collides with another active resident script's `/Scripting/Script<N>` path. Because our
    // script is started via `org.kde.kwin.Scripting.start()`, `unloadScript(pluginName)` cleanly
    // stops, deletes, and unregisters our script by exact `pluginName`.
    const int scriptId = (memId >= 0) ? memId : fileId;
    if (isLoaded || scriptId >= 0) {
        QDBusMessage unloadMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("unloadScript")
        );
        unloadMsg << pluginName;
        bus.call(unloadMsg);
    }
    QFile::remove(resident_kwin_js_path());
}

inline QString build_kwin_resident_script_js(
    const QString& dbusAddr,
    int initOvX,
    int initOvY,
    int initOvW,
    int initOvH,
    const QString& dbusIface
) {
    return QStringLiteral(
        "var dbusAddr = '%1';\n"
        "var dbusIface = '%6';\n"
        "var targetOvX = %2;\n"
        "var targetOvY = %3;\n"
        "var targetOvW = %4;\n"
        "var targetOvH = %5;\n"
        "var activeWindows = {};\n"
        "var captionHookedIds = {};\n"
        "var hookedOverlayIds = {};\n"
        "var hookedOverlayTime = {};\n"
        "var applyingOverlayTime = {};\n"
        "var hookedMainIds = {};\n"
        "var movingOverlayIds = {};\n"
        "var movingMainIds = {};\n"
        "function windowListAll() {\n"
        "  return workspace.stackingOrder || workspace.windowList();\n"
        "}\n"
        "function isLibreLinkUpCandidate(w) {\n"
        "  if (!w) return false;\n"
        "  var cap = w.caption ? w.caption.toString() : '';\n"
        "  if (cap.indexOf('LibreLinkUp') === 0) return true;\n"
        "  var rc = w.resourceClass ? w.resourceClass.toString().toLowerCase() : '';\n"
        "  if (rc.indexOf('librelinkup') !== -1) return true;\n"
        "  var df = w.desktopFileName ? w.desktopFileName.toString().toLowerCase() : '';\n"
        "  return df.indexOf('librelinkup') !== -1;\n"
        "}\n"
        "function isOverlayWin(w) {\n"
        "  if (!w) return false;\n"
        "  var cap = w.caption ? w.caption.toString() : '';\n"
        "  return cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0 && cap.indexOf('Tooltip') === -1;\n"
        "}\n"
        "function isMainWin(w) {\n"
        "  if (!w) return false;\n"
        "  var cap = w.caption ? w.caption.toString() : '';\n"
        "  return cap.indexOf('LibreLinkUp Rust Desktop v') === 0;\n"
        "}\n"
        "function windowKey(w) {\n"
        "  return (w && w.internalId) ? w.internalId.toString() : '';\n"
        "}\n"
        "function applyOverlayFrame(w, wid, gx, gy, gw, gh) {\n"
        "  if (!w || w.move || w.resize || gw <= 0 || gh <= 0 || isNaN(gx) || isNaN(gy)) return;\n"
        "  if (wid) applyingOverlayTime[wid] = Date.now();\n"
        "  w.frameGeometry = { x: gx, y: gy, width: gw, height: gh };\n"
        "}\n"
        "function reportOverlayGeometry(w) {\n"
        "  if (!w || w.move || w.resize) return;\n"
        "  var outName = (w.output && w.output.name) ? w.output.name.toString() : '';\n"
        "  var fg = w.frameGeometry;\n"
        "  if (dbusAddr && fg && fg.width > 0 && fg.height > 0) {\n"
        "    targetOvX = Math.round(fg.x);\n"
        "    targetOvY = Math.round(fg.y);\n"
        "    targetOvW = Math.round(fg.width);\n"
        "    targetOvH = Math.round(fg.height);\n"
        "    callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'onKWinOverlayGeometry',\n"
        "      outName, targetOvX, targetOvY, targetOvW, targetOvH);\n"
        "  }\n"
        "}\n"
        "function reportMainGeometry(w) {\n"
        "  if (!w || w.move || w.resize) return;\n"
        "  var outName = (w.output && w.output.name) ? w.output.name.toString() : '';\n"
        "  var fg = w.frameGeometry;\n"
        "  if (dbusAddr && fg && fg.width > 0 && fg.height > 0) {\n"
        "    callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'onKWinMainWindowGeometry',\n"
        "      outName, Math.round(fg.x), Math.round(fg.y), Math.round(fg.width), Math.round(fg.height));\n"
        "  }\n"
        "}\n"
        "function enforceLibreLinkUpRules(w) {\n"
        "  if (!w || !isLibreLinkUpCandidate(w)) return;\n"
        "  var wid = windowKey(w);\n"
        "  var cap = w.caption ? w.caption.toString() : '';\n"
        "  if (wid && !captionHookedIds[wid]) {\n"
        "    captionHookedIds[wid] = true;\n"
        "    if (w.captionChanged) {\n"
        "      w.captionChanged.connect(function() {\n"
        "        enforceLibreLinkUpRules(w);\n"
        "      });\n"
        "    }\n"
        "    if (w.closed) {\n"
        "      w.closed.connect(function() {\n"
        "        delete captionHookedIds[wid];\n"
        "        delete activeWindows[wid];\n"
        "        delete hookedOverlayIds[wid];\n"
        "        delete hookedOverlayTime[wid];\n"
        "        delete applyingOverlayTime[wid];\n"
        "        delete hookedMainIds[wid];\n"
        "        delete movingOverlayIds[wid];\n"
        "        delete movingMainIds[wid];\n"
        "      });\n"
        "    }\n"
        "  }\n"
        "  if (cap.length === 0) {\n"
        "    var fg0 = w.frameGeometry;\n"
        "    if (!w.popupWindow && !w.tooltip && !w.transient\n"
        "        && fg0 && fg0.width >= 220 && fg0.width <= 620 && fg0.height >= 72 && fg0.height <= 280) {\n"
        "      if (!w.noBorder) w.noBorder = true;\n"
        "      if (!w.keepAbove) w.keepAbove = true;\n"
        "      if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "      if (!w.skipPager) w.skipPager = true;\n"
        "      if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "      applyOverlayFrame(w, wid, targetOvX, targetOvY, targetOvW, targetOvH);\n"
        "    }\n"
        "    return;\n"
        "  }\n"
        "  if (cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0) {\n"
        "    var isTooltip = (cap.indexOf('Tooltip') !== -1);\n"
        "    if (isTooltip) return;\n"
        "    var allDesk = (cap.indexOf('[SingleDesktop]') === -1);\n"
        "    if (!w.noBorder) w.noBorder = true;\n"
        "    if (!w.keepAbove) w.keepAbove = true;\n"
        "    if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "    if (!w.skipPager) w.skipPager = true;\n"
        "    if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "    if (w.onAllDesktops !== allDesk) w.onAllDesktops = allDesk;\n"
        "    if (w.minimized) {\n"
        "      w.minimized = false;\n"
        "    }\n"
        "    if (wid && !hookedOverlayIds[wid]) {\n"
        "      hookedOverlayIds[wid] = true;\n"
        "      hookedOverlayTime[wid] = Date.now();\n"
        "      activeWindows[wid] = w;\n"
        "      applyOverlayFrame(w, wid, targetOvX, targetOvY, targetOvW, targetOvH);\n"
        "      if (w.skipTaskbarChanged) {\n"
        "        w.skipTaskbarChanged.connect(function() {\n"
        "          if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "        });\n"
        "      }\n"
        "      if (w.skipPagerChanged) {\n"
        "        w.skipPagerChanged.connect(function() {\n"
        "          if (!w.skipPager) w.skipPager = true;\n"
        "        });\n"
        "      }\n"
        "      if (w.skipSwitcherChanged) {\n"
        "        w.skipSwitcherChanged.connect(function() {\n"
        "          if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "        });\n"
        "      }\n"
        "      if (w.keepAboveChanged) {\n"
        "        w.keepAboveChanged.connect(function() {\n"
        "          if (!w.keepAbove) w.keepAbove = true;\n"
        "        });\n"
        "      }\n"
        "      if (w.noBorderChanged) {\n"
        "        w.noBorderChanged.connect(function() {\n"
        "          if (!w.noBorder) w.noBorder = true;\n"
        "        });\n"
        "      }\n"
        "      if (w.minimizedChanged) {\n"
        "        w.minimizedChanged.connect(function() {\n"
        "          if (w.minimized) {\n"
        "            w.minimized = false;\n"
        "            if (!w.keepAbove) w.keepAbove = true;\n"
        "            if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "          }\n"
        "        });\n"
        "      }\n"
        "      if (w.interactiveMoveResizeFinished) {\n"
        "        w.interactiveMoveResizeFinished.connect(function() {\n"
        "          delete movingOverlayIds[wid];\n"
        "          reportOverlayGeometry(w);\n"
        "        });\n"
        "      }\n"
        "      if (w.moveResizedChanged) {\n"
        "        w.moveResizedChanged.connect(function() {\n"
        "          if (w.move || w.resize) {\n"
        "            movingOverlayIds[wid] = true;\n"
        "          } else if (movingOverlayIds[wid]) {\n"
        "            delete movingOverlayIds[wid];\n"
        "            reportOverlayGeometry(w);\n"
        "          }\n"
        "        });\n"
        "      }\n"
        "      if (w.frameGeometryChanged) {\n"
        "        w.frameGeometryChanged.connect(function() {\n"
        "          if (w.move || w.resize || movingOverlayIds[wid]) return;\n"
        "          var now = Date.now();\n"
        "          if (now - (hookedOverlayTime[wid] || 0) < 450) return;\n"
        "          if (now - (applyingOverlayTime[wid] || 0) < 180) return;\n"
        "          var fg = w.frameGeometry;\n"
        "          if (!fg) return;\n"
        "          if (Math.abs(Math.round(fg.x) - targetOvX) > 2\n"
        "              || Math.abs(Math.round(fg.y) - targetOvY) > 2\n"
        "              || Math.abs(Math.round(fg.width) - targetOvW) > 2\n"
        "              || Math.abs(Math.round(fg.height) - targetOvH) > 2) {\n"
        "            reportOverlayGeometry(w);\n"
        "          }\n"
        "        });\n"
        "      }\n"
        "    }\n"
        "    return;\n"
        "  }\n"
        "  if (cap.indexOf('LibreLinkUp Rust Desktop v') === 0) {\n"
        "    if (w.minimized) {\n"
        "      if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "      if (!w.skipPager) w.skipPager = true;\n"
        "      if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "    } else {\n"
        "      if (w.skipTaskbar) w.skipTaskbar = false;\n"
        "      if (w.skipPager) w.skipPager = false;\n"
        "      if (w.skipSwitcher) w.skipSwitcher = false;\n"
        "    }\n"
        "    if (wid && !hookedMainIds[wid]) {\n"
        "      hookedMainIds[wid] = true;\n"
        "      activeWindows[wid] = w;\n"
        "      if (w.minimizedChanged) {\n"
        "        w.minimizedChanged.connect(function() {\n"
        "          if (w.minimized) {\n"
        "            if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "            if (!w.skipPager) w.skipPager = true;\n"
        "            if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "          } else {\n"
        "            if (w.skipTaskbar) w.skipTaskbar = false;\n"
        "            if (w.skipPager) w.skipPager = false;\n"
        "            if (w.skipSwitcher) w.skipSwitcher = false;\n"
        "          }\n"
        "        });\n"
        "      }\n"
        "      if (w.interactiveMoveResizeFinished) {\n"
        "        w.interactiveMoveResizeFinished.connect(function() {\n"
        "          delete movingMainIds[wid];\n"
        "          reportMainGeometry(w);\n"
        "        });\n"
        "      }\n"
        "      if (w.moveResizedChanged) {\n"
        "        w.moveResizedChanged.connect(function() {\n"
        "          if (w.move || w.resize) {\n"
        "            movingMainIds[wid] = true;\n"
        "          } else if (movingMainIds[wid]) {\n"
        "            delete movingMainIds[wid];\n"
        "            reportMainGeometry(w);\n"
        "          }\n"
        "        });\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
        "var list = windowListAll();\n"
        "if (list) {\n"
        "  for (var i = 0; i < list.length; ++i) {\n"
        "    enforceLibreLinkUpRules(list[i]);\n"
        "  }\n"
        "}\n"
        "if (workspace.windowAdded) {\n"
        "  workspace.windowAdded.connect(function(w) {\n"
        "    enforceLibreLinkUpRules(w);\n"
        "  });\n"
        "}\n"
        "registerShortcut('LibreLinkUp SyncOverlay', 'LibreLinkUp (internal): Sync Overlay Window', '', function() {\n"
        "  if (!dbusAddr) return;\n"
        "  callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'getDesiredOverlaySync', function(spec) {\n"
        "    if (!spec) return;\n"
        "    var parts = spec.toString().split(',');\n"
        "    if (parts.length < 5) return;\n"
        "    var gx = parseInt(parts[0], 10);\n"
        "    var gy = parseInt(parts[1], 10);\n"
        "    var gw = parseInt(parts[2], 10);\n"
        "    var gh = parseInt(parts[3], 10);\n"
        "    var allDesk = (parts[4] === '1' || parts[4] === 'true');\n"
        "    if (gw > 0 && gh > 0 && !isNaN(gx) && !isNaN(gy)) {\n"
        "      targetOvX = gx;\n"
        "      targetOvY = gy;\n"
        "      targetOvW = gw;\n"
        "      targetOvH = gh;\n"
        "    }\n"
        "    var wl = windowListAll();\n"
        "    if (!wl) return;\n"
        "    for (var i = 0; i < wl.length; ++i) {\n"
        "      var w = wl[i];\n"
        "      if (isOverlayWin(w)) {\n"
        "        var wid = windowKey(w);\n"
        "        if (!w.noBorder) w.noBorder = true;\n"
        "        if (w.onAllDesktops !== allDesk) w.onAllDesktops = allDesk;\n"
        "        if (!w.keepAbove) w.keepAbove = true;\n"
        "        if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "        if (!w.skipPager) w.skipPager = true;\n"
        "        if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "        if (w.minimized) w.minimized = false;\n"
        "        applyOverlayFrame(w, wid, gx, gy, gw, gh);\n"
        "      }\n"
        "    }\n"
        "  });\n"
        "});\n"
        "registerShortcut('LibreLinkUp SyncMain', 'LibreLinkUp (internal): Sync Main Window', '', function() {\n"
        "  if (!dbusAddr) return;\n"
        "  callDBus(dbusAddr, '/LibreLinkUpKWin', dbusIface, 'getDesiredMainSync', function(spec) {\n"
        "    if (!spec) return;\n"
        "    var parts = spec.toString().split(',');\n"
        "    if (parts.length < 4) return;\n"
        "    var gx = parseInt(parts[0], 10);\n"
        "    var gy = parseInt(parts[1], 10);\n"
        "    var gw = parseInt(parts[2], 10);\n"
        "    var gh = parseInt(parts[3], 10);\n"
        "    var wl = windowListAll();\n"
        "    if (!wl) return;\n"
        "    for (var i = 0; i < wl.length; ++i) {\n"
        "      var w = wl[i];\n"
        "      if (isMainWin(w)) {\n"
        "        if (!w.move && !w.resize && gw > 0 && gh > 0 && !isNaN(gx) && !isNaN(gy)) {\n"
        "          w.frameGeometry = { x: gx, y: gy, width: gw, height: gh };\n"
        "        }\n"
        "        workspace.activeWindow = w;\n"
        "      }\n"
        "    }\n"
        "  });\n"
        "});\n"
        "registerShortcut('LibreLinkUp QueryOverlay', 'LibreLinkUp (internal): Query Overlay Geometry', '', function() {\n"
        "  var wl = windowListAll();\n"
        "  if (!wl) return;\n"
        "  for (var i = 0; i < wl.length; ++i) {\n"
        "    var w = wl[i];\n"
        "    if (isOverlayWin(w)) {\n"
        "      reportOverlayGeometry(w);\n"
        "    }\n"
        "  }\n"
        "});\n"
        "registerShortcut('LibreLinkUp QueryMain', 'LibreLinkUp (internal): Query Main Geometry', '', function() {\n"
        "  var wl = windowListAll();\n"
        "  if (!wl) return;\n"
        "  for (var i = 0; i < wl.length; ++i) {\n"
        "    var w = wl[i];\n"
        "    if (isMainWin(w)) {\n"
        "      reportMainGeometry(w);\n"
        "    }\n"
        "  }\n"
        "});\n"
    ).arg(dbusAddr).arg(initOvX).arg(initOvY).arg(initOvW).arg(initOvH).arg(dbusIface);
}

inline void install_kwin_resident_script() {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    const QString pluginName = QStringLiteral("librelinkup_kwin_resident_watcher");
    stop_and_unload_resident_script();

    int initOvX = -1;
    int initOvY = -1;
    int initOvW = 0;
    int initOvH = 0;
    {
        const auto& st = OverlayPlacementState::instance();
        if (st.hasPlacement) {
            QScreen* targetSc = nullptr;
            int gx = 0, gy = 0, gw = 276, gh = 88;
            if (st.computeGlobalGeometry(nullptr, targetSc, gx, gy, gw, gh)) {
                initOvX = gx;
                initOvY = gy;
                initOvW = gw;
                initOvH = gh;
            }
        }
    }

    const QString dbusAddr = kwin_dbus_service_name_ref().isEmpty()
        ? bus.baseService()
        : kwin_dbus_service_name_ref();
    const QString jsBody = build_kwin_resident_script_js(
        dbusAddr,
        initOvX,
        initOvY,
        initOvW,
        initOvH,
        kwin_dbus_interface_name()
    );

    const QString scriptPath = resident_kwin_js_path();
    QFile f(scriptPath);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return;
    }
    f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    const bool wrote = f.write(jsBody.toUtf8()) >= 0;
    f.close();
    if (!wrote) {
        QFile::remove(scriptPath);
        return;
    }

    QDBusMessage loadMsg = QDBusMessage::createMethodCall(
        QStringLiteral("org.kde.KWin"),
        QStringLiteral("/Scripting"),
        QStringLiteral("org.kde.kwin.Scripting"),
        QStringLiteral("loadScript")
    );
    loadMsg << scriptPath << pluginName;
    QDBusReply<int> loadReply = bus.call(loadMsg);
    if (loadReply.isValid() && loadReply.value() >= 0) {
        const int sid = loadReply.value();
        resident_kwin_script_id_ref() = sid;
        {
            QFile sf(resident_kwin_sid_path());
            if (sf.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
                sf.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
                sf.write(QString::number(sid).toUtf8());
                sf.close();
            }
        }
        // Start via `/Scripting` `org.kde.kwin.Scripting.start` rather than `/Scripting/Script<sid>`
        // `run()`, because KWin 6's `scripts.size()` index can collide with an already-occupied
        // `/Scripting/Script<N>` D-Bus path when an earlier script slot was unloaded.
        QDBusMessage startMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("start")
        );
        bus.call(startMsg);
    } else {
        QFile::remove(scriptPath);
    }
}

inline void ensure_kwin_dbus_bridge() {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    static bool s_registered = false;
    if (s_registered || !QCoreApplication::instance()) {
        return;
    }
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    s_registered = true;
    auto* watcher = new NotificationActionWatcher(QCoreApplication::instance());
    bus.registerObject(
        QStringLiteral("/LibreLinkUpKWin"),
        watcher,
        QDBusConnection::ExportScriptableSlots
    );
    // Register our owned well-known D-Bus service name (`io.github.russellmilliner.LibreLinkUpDesktop`)
    // so Flatpak's xdg-dbus-proxy allows host KWin scripts (`callDBus`) to invoke `/LibreLinkUpKWin`
    // methods bidirectionally (xdg-dbus-proxy drops inbound host calls to unnamed `:1.xxx` connections).
    const QString wellKnownService = QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop");
    if (bus.registerService(wellKnownService)) {
        kwin_dbus_service_name_ref() = wellKnownService;
    } else {
        kwin_dbus_service_name_ref() = bus.baseService();
    }

    // Clean up any leftover temporary KWin script files from prior runs
    QDir dir(kwin_script_dir());
    const QStringList stale = dir.entryList(
        QStringList{QStringLiteral(".librelinkup-kwin-*.js"), QStringLiteral("librelinkup-kwin-*.js")},
        QDir::Files | QDir::Hidden
    );
    for (const QString& name : stale) {
        QFile::remove(dir.filePath(name));
    }

    install_kwin_resident_script();

    // If kwin_wayland restarts during the session, automatically reload the resident watcher
    // and re-apply overlay placement/taskbar-exclusion rules.
    auto* kwinServiceWatcher = new QDBusServiceWatcher(
        QStringLiteral("org.kde.KWin"),
        bus,
        QDBusServiceWatcher::WatchForRegistration,
        QCoreApplication::instance()
    );
    QObject::connect(kwinServiceWatcher, &QDBusServiceWatcher::serviceRegistered, kwinServiceWatcher, [](const QString&) {
        resident_kwin_script_id_ref() = -1;
        QFile::remove(resident_kwin_sid_path());
        OverlayPlacementState::instance().invalidateKWinSync();
        QTimer::singleShot(250, []() {
            install_kwin_resident_script();
            if (QWindow* ovWin = find_overlay_window()) {
                if (ovWin->isVisible() && OverlayPlacementState::instance().hasPlacement) {
                    OverlayPlacementState::instance().invalidateKWinSync();
                    OverlayPlacementState::instance().applyToWindow(ovWin);
                }
            }
        });
    });

    QObject::connect(QCoreApplication::instance(), &QCoreApplication::aboutToQuit, []() {
        stop_and_unload_resident_script();
    });
}

inline bool invoke_kwin_resident_shortcut(const QString& shortcutName) {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return true;
    }
    ensure_kwin_dbus_bridge();
    if (resident_kwin_script_id_ref() < 0) {
        return false;
    }
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return false;
    }
    QDBusMessage msg = QDBusMessage::createMethodCall(
        QStringLiteral("org.kde.kglobalaccel"),
        QStringLiteral("/component/kwin"),
        QStringLiteral("org.kde.kglobalaccel.Component"),
        QStringLiteral("invokeShortcut")
    );
    msg << shortcutName;
    const QDBusMessage reply = bus.call(msg);
    return reply.type() != QDBusMessage::ErrorMessage;
}

inline void run_kwin_oneshot_script(const QString& jsBody) {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    ensure_kwin_dbus_bridge();
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    const QString uid = QUuid::createUuid().toString(QUuid::Id128);
    const QString scriptPath = kwin_script_dir()
        + QStringLiteral("/.librelinkup-kwin-%1.js").arg(uid);
    QFile f(scriptPath);
    if (!f.open(QIODevice::NewOnly | QIODevice::WriteOnly | QIODevice::Text)) {
        return;
    }
    f.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
    const bool wrote = f.write(jsBody.toUtf8()) >= 0;
    f.close();
    if (!wrote) {
        QFile::remove(scriptPath);
        return;
    }

    const QString pluginName = QStringLiteral("librelinkup_kwin_%1").arg(uid);
    QDBusMessage loadMsg = QDBusMessage::createMethodCall(
        QStringLiteral("org.kde.KWin"),
        QStringLiteral("/Scripting"),
        QStringLiteral("org.kde.kwin.Scripting"),
        QStringLiteral("loadScript")
    );
    loadMsg << scriptPath << pluginName;
    QDBusReply<int> loadReply = bus.call(loadMsg);
    if (loadReply.isValid() && loadReply.value() >= 0) {
        QDBusMessage startMsg = QDBusMessage::createMethodCall(
            QStringLiteral("org.kde.KWin"),
            QStringLiteral("/Scripting"),
            QStringLiteral("org.kde.kwin.Scripting"),
            QStringLiteral("start")
        );
        bus.call(startMsg);
        QTimer::singleShot(300, [pluginName, scriptPath]() {
            QDBusConnection session = QDBusConnection::sessionBus();
            if (session.isConnected()) {
                QDBusMessage cleanupMsg = QDBusMessage::createMethodCall(
                    QStringLiteral("org.kde.KWin"),
                    QStringLiteral("/Scripting"),
                    QStringLiteral("org.kde.kwin.Scripting"),
                    QStringLiteral("unloadScript")
                );
                cleanupMsg << pluginName;
                session.call(cleanupMsg);
            }
            QFile::remove(scriptPath);
        });
    } else {
        QFile::remove(scriptPath);
    }
}

inline void sync_kwin_overlay_window(int globalX, int globalY, int w, int h, bool onAllDesktops) {
    auto& desired = DesiredKWinSyncState::instance();
    desired.ovX = globalX;
    desired.ovY = globalY;
    desired.ovW = w;
    desired.ovH = h;
    desired.ovAllDesktops = onAllDesktops;
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }

    if (invoke_kwin_resident_shortcut(QStringLiteral("LibreLinkUp SyncOverlay"))) {
        return;
    }

    const QString js = QStringLiteral(
        "var list = workspace.stackingOrder || workspace.windowList();\n"
        "if (list) {\n"
        "  for (var i = 0; i < list.length; ++i) {\n"
        "    var w = list[i];\n"
        "    var cap = w && w.caption ? w.caption.toString() : '';\n"
        "    if (cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0 && cap.indexOf('Tooltip') === -1) {\n"
        "      if (!w.noBorder) w.noBorder = true;\n"
        "      if (w.onAllDesktops !== %1) w.onAllDesktops = %1;\n"
        "      if (!w.keepAbove) w.keepAbove = true;\n"
        "      if (!w.skipTaskbar) w.skipTaskbar = true;\n"
        "      if (!w.skipPager) w.skipPager = true;\n"
        "      if (!w.skipSwitcher) w.skipSwitcher = true;\n"
        "      if (!w.move && !w.resize && %2 > 0 && %3 > 0) {\n"
        "        w.frameGeometry = { x: %4, y: %5, width: %2, height: %3 };\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    )
        .arg(onAllDesktops ? QStringLiteral("true") : QStringLiteral("false"))
        .arg(w)
        .arg(h)
        .arg(globalX)
        .arg(globalY);
    run_kwin_oneshot_script(js);
}

inline void sync_kwin_main_window(int globalX, int globalY, int w, int h) {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    auto& desired = DesiredKWinSyncState::instance();
    desired.mainX = globalX;
    desired.mainY = globalY;
    desired.mainW = w;
    desired.mainH = h;

    if (invoke_kwin_resident_shortcut(QStringLiteral("LibreLinkUp SyncMain"))) {
        return;
    }

    const QString js = QStringLiteral(
        "var list = workspace.stackingOrder || workspace.windowList();\n"
        "if (list) {\n"
        "  for (var i = 0; i < list.length; ++i) {\n"
        "    var w = list[i];\n"
        "    var cap = w && w.caption ? w.caption.toString() : '';\n"
        "    if (cap.indexOf('LibreLinkUp Rust Desktop v') === 0) {\n"
        "      if (!w.move && !w.resize && %1 > 0 && %2 > 0) {\n"
        "        w.frameGeometry = { x: %3, y: %4, width: %1, height: %2 };\n"
        "      }\n"
        "      workspace.activeWindow = w;\n"
        "    }\n"
        "  }\n"
        "}\n"
    )
        .arg(w)
        .arg(h)
        .arg(globalX)
        .arg(globalY);
    run_kwin_oneshot_script(js);
}

inline void query_kwin_overlay_geometry() {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    if (invoke_kwin_resident_shortcut(QStringLiteral("LibreLinkUp QueryOverlay"))) {
        return;
    }

    QDBusConnection bus = QDBusConnection::sessionBus();
    const QString dbusAddr = kwin_dbus_service_name_ref().isEmpty()
        ? bus.baseService()
        : kwin_dbus_service_name_ref();
    if (dbusAddr.isEmpty()) {
        return;
    }
    const QString js = QStringLiteral(
        "var list = workspace.stackingOrder || workspace.windowList();\n"
        "if (list) {\n"
        "  for (var i = 0; i < list.length; ++i) {\n"
        "    var w = list[i];\n"
        "    var cap = w && w.caption ? w.caption.toString() : '';\n"
        "    if (cap.indexOf('LibreLinkUp Rust Desktop Overlay') === 0 && cap.indexOf('Tooltip') === -1) {\n"
        "      if (!w.move && !w.resize) {\n"
        "        var outName = (w.output && w.output.name) ? w.output.name.toString() : '';\n"
        "        var fg = w.frameGeometry;\n"
        "        callDBus('%1', '/LibreLinkUpKWin', '%2', 'onKWinOverlayGeometry',\n"
        "          outName, Math.round(fg.x), Math.round(fg.y), Math.round(fg.width), Math.round(fg.height));\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    ).arg(dbusAddr, kwin_dbus_interface_name());
    run_kwin_oneshot_script(js);
}

inline void query_kwin_main_window_geometry() {
    if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
        return;
    }
    if (invoke_kwin_resident_shortcut(QStringLiteral("LibreLinkUp QueryMain"))) {
        return;
    }

    QDBusConnection bus = QDBusConnection::sessionBus();
    const QString dbusAddr = kwin_dbus_service_name_ref().isEmpty()
        ? bus.baseService()
        : kwin_dbus_service_name_ref();
    if (dbusAddr.isEmpty()) {
        return;
    }
    const QString js = QStringLiteral(
        "var list = workspace.stackingOrder || workspace.windowList();\n"
        "if (list) {\n"
        "  for (var i = 0; i < list.length; ++i) {\n"
        "    var w = list[i];\n"
        "    var cap = w && w.caption ? w.caption.toString() : '';\n"
        "    if (cap.indexOf('LibreLinkUp Rust Desktop v') === 0) {\n"
        "      if (!w.move && !w.resize) {\n"
        "        var outName = (w.output && w.output.name) ? w.output.name.toString() : '';\n"
        "        var fg = w.frameGeometry;\n"
        "        callDBus('%1', '/LibreLinkUpKWin', '%2', 'onKWinMainWindowGeometry',\n"
        "          outName, Math.round(fg.x), Math.round(fg.y), Math.round(fg.width), Math.round(fg.height));\n"
        "      }\n"
        "    }\n"
        "  }\n"
        "}\n"
    ).arg(dbusAddr, kwin_dbus_interface_name());
    run_kwin_oneshot_script(js);
}

inline void handle_kwin_overlay_geometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    QScreen* sc = nullptr;
    const QString cleanName = screenName.trimmed();
    if (!cleanName.isEmpty()) {
        for (QScreen* cand : QGuiApplication::screens()) {
            if (cand && cand->name().compare(cleanName, Qt::CaseInsensitive) == 0) {
                sc = cand;
                break;
            }
        }
    }
    if (!sc) {
        sc = QGuiApplication::screenAt(QPoint(globalX + width / 2, globalY + height / 2));
    }
    if (!sc) {
        sc = QGuiApplication::primaryScreen();
    }
    if (!sc) {
        return;
    }
    const QRect scGeom = sc->geometry();
    const int relX = std::max(0, globalX - scGeom.x());
    const int relY = std::max(0, globalY - scGeom.y());
    const int w = width > 0 ? width : 276;
    const int h = height > 0 ? height : 88;

    auto& st = OverlayPlacementState::instance();
    st.screenName = sc->name();
    st.relX = relX;
    st.relY = relY;
    st.width = w;
    st.height = h;
    st.hasPlacement = true;
    st.dragging = false;
    st.systemMoveStarted = false;
    st.lastKWinX = globalX;
    st.lastKWinY = globalY;
    st.lastKWinW = w;
    st.lastKWinH = h;
    st.lastKWinAllDesktops = st.onAllDesktops;
    st.lastKWinSynced = true;

    auto& desired = DesiredKWinSyncState::instance();
    desired.ovX = globalX;
    desired.ovY = globalY;
    desired.ovW = w;
    desired.ovH = h;
    desired.ovAllDesktops = st.onAllDesktops;

    if (QWindow* mainWin = find_main_window()) {
        if (QObject* settingsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "saveOverlayPlacement",
                Qt::DirectConnection,
                Q_ARG(QString, sc->name()),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relX)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relY)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(w)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(h))
            );
        }
    }
}

inline void handle_kwin_main_window_geometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    auto& st = MainWindowPlacementState::instance();
    if (st.applying) {
        return;
    }
    QScreen* sc = nullptr;
    const QString cleanName = screenName.trimmed();
    if (!cleanName.isEmpty()) {
        for (QScreen* cand : QGuiApplication::screens()) {
            if (cand && cand->name().compare(cleanName, Qt::CaseInsensitive) == 0) {
                sc = cand;
                break;
            }
        }
    }
    if (!sc) {
        sc = QGuiApplication::screenAt(QPoint(globalX + width / 2, globalY + height / 2));
    }
    if (!sc) {
        sc = QGuiApplication::primaryScreen();
    }
    if (!sc) {
        return;
    }
    const QRect scGeom = sc->geometry();
    const int relX = std::max(0, globalX - scGeom.x());
    const int relY = std::max(0, globalY - scGeom.y());
    const int w = std::max(780, width);
    const int h = std::max(540, height);

    st.screenName = sc->name();
    st.relX = relX;
    st.relY = relY;
    st.width = w;
    st.height = h;
    st.hasPlacement = true;

    if (QWindow* mainWin = find_main_window()) {
        if (QObject* settingsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "saveMainWindowPlacement",
                Qt::DirectConnection,
                Q_ARG(QString, sc->name()),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relX)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relY)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(w)),
                Q_ARG(::std::int32_t, static_cast<::std::int32_t>(h))
            );
        }
    }
}

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
    if (QWindow* tipWin = ovWin->findChild<QWindow*>(QStringLiteral("librelinkupOverlayTooltipWindow"))) {
        if (tipWin->transientParent() != ovWin) {
            tipWin->setTransientParent(ovWin);
        }
        QObject::connect(tipWin, &QWindow::transientParentChanged, tipWin, [tipWin, ovWin](QWindow* parent) {
            if (parent != ovWin) {
                tipWin->setTransientParent(ovWin);
            }
        });
    }
    auto applyOverlayPlacementAndKWin = [ovWin]() {
        if (!ovWin || !ovWin->isVisible()) {
            return;
        }
        if (OverlayPlacementState::instance().hasPlacement) {
            OverlayPlacementState::instance().applyToWindow(ovWin, true);
            for (int delayMs : {35, 110, 260}) {
                QTimer::singleShot(delayMs, ovWin, [ovWin]() {
                    if (ovWin && ovWin->isVisible() && !OverlayPlacementState::instance().dragging) {
                        OverlayPlacementState::instance().applyToWindow(ovWin, true);
                    }
                });
            }
        } else {
            clamp_window_to_screen(ovWin);
        }
    };
    QObject::connect(ovWin, &QWindow::visibleChanged, ovWin, [ovWin, applyOverlayPlacementAndKWin](bool vis) {
        OverlayPlacementState::instance().invalidateKWinSync();
        if (vis) {
            applyOverlayPlacementAndKWin();
        } else if (ovWin && OverlayPlacementState::instance().hasPlacement) {
            // Pre-target the saved QScreen while hidden so the next Wayland surface creation
            // already hints the saved wl_output before KWin snaps the exact frameGeometry.
            OverlayPlacementState::instance().applyToWindow(ovWin, false);
        }
        update_tray_window_actions();
    });
    if (ovWin->isVisible()) {
        applyOverlayPlacementAndKWin();
    }
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
    const QString cleanScreen = screen_name.trimmed();
    st.invalidateKWinSync();
    st.screenName = cleanScreen;
    st.relX = x;
    st.relY = y;
    st.width = width;
    st.height = height;
    st.hasPlacement = true;

    QScreen* targetSc = nullptr;
    int gx = 0, gy = 0, gw = 276, gh = 88;
    const bool hasGeom = st.computeGlobalGeometry(nullptr, targetSc, gx, gy, gw, gh);
    if (hasGeom) {
        auto& desired = DesiredKWinSyncState::instance();
        desired.ovX = gx;
        desired.ovY = gy;
        desired.ovW = gw;
        desired.ovH = gh;
        desired.ovAllDesktops = st.onAllDesktops;
    }

    if (is_wayland_platform()) {
        ensure_kwin_dbus_bridge();
        if (hasGeom) {
            sync_kwin_overlay_window(gx, gy, gw, gh, st.onAllDesktops);
        }
    }

    if (QWindow* ovWin = find_overlay_window()) {
        st.applyToWindow(ovWin, true);
    }
}

inline void apply_overlay_all_desktops(bool onAllDesktops) {
    auto& st = OverlayPlacementState::instance();
    if (st.onAllDesktops != onAllDesktops) {
        st.invalidateKWinSync();
    }
    st.onAllDesktops = onAllDesktops;

    if (QWindow* ovWin = find_overlay_window()) {
        if (!is_wayland_platform()) {
            const Qt::WindowFlags desiredFlags = Qt::Tool
                | Qt::FramelessWindowHint
                | Qt::WindowStaysOnTopHint
                | (onAllDesktops ? Qt::BypassWindowManagerHint : Qt::WindowFlags());
            if (ovWin->flags() != desiredFlags) {
                const bool wasVis = ovWin->isVisible();
                if (wasVis) {
                    ovWin->hide();
                }
                ovWin->setFlags(desiredFlags);
                if (wasVis) {
                    st.applyToWindow(ovWin);
                    ovWin->show();
                    st.applyToWindow(ovWin);
                }
            }
        } else if (ovWin->isVisible() && st.hasPlacement) {
            st.applyToWindow(ovWin);
            return;
        }
    }

    if (is_wayland_platform() && (!st.lastKWinSynced || st.lastKWinAllDesktops != onAllDesktops)) {
        st.lastKWinAllDesktops = onAllDesktops;
        sync_kwin_overlay_window(0, 0, 0, 0, onAllDesktops);
    }
}

inline void begin_overlay_drag() {
    QWindow* ovWin = find_overlay_window();
    if (!ovWin) {
        return;
    }
    auto& st = OverlayPlacementState::instance();
    st.systemMoveStarted = false;
    if (is_wayland_platform()) {
        st.dragging = false;
        return;
    }
    st.dragging = true;
    st.dragStartCursor = QCursor::pos();
    st.dragStartWinPos = ovWin->position();
}

inline void update_overlay_drag() {
    QWindow* ovWin = find_overlay_window();
    auto& st = OverlayPlacementState::instance();
    if (!ovWin) {
        return;
    }
    if (is_wayland_platform()) {
        if (!st.systemMoveStarted) {
            st.systemMoveStarted = true;
            st.dragging = true;
            ensure_kwin_dbus_bridge();
            ovWin->startSystemMove();
        }
        return;
    }
    if (!st.dragging) {
        return;
    }
    const QPoint nextPos = st.dragStartWinPos + (QCursor::pos() - st.dragStartCursor);
    ovWin->setPosition(nextPos);
}

inline void end_overlay_drag(QObject* controller) {
    auto& st = OverlayPlacementState::instance();
    if (is_wayland_platform()) {
        // On Wayland, ovWin->startSystemMove() transfers the pointer grab to KWin, which
        // immediately causes Qt Quick's MouseArea to emit onCanceled (~1ms into the drag).
        // The resident KWin watcher listens to interactiveMoveResizeFinished and
        // moveResizedChanged and reports the final geometry via onKWinOverlayGeometry once
        // the user releases the mouse button—without spawning any one-shot KWin scripts.
        return;
    }
    st.dragging = false;
    st.systemMoveStarted = false;
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
            Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relX)),
            Q_ARG(::std::int32_t, static_cast<::std::int32_t>(relY)),
            Q_ARG(::std::int32_t, static_cast<::std::int32_t>(w)),
            Q_ARG(::std::int32_t, static_cast<::std::int32_t>(h))
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
        if (is_wayland_platform()) {
            for (int delayMs : {25, 90, 200}) {
                QTimer::singleShot(delayMs, win, [win]() {
                    if (win && win->isVisible()) {
                        MainWindowPlacementState::instance().applyToWindow(win, true);
                    }
                });
            }
        } else {
            win->raise();
        }
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

inline void show_main_window_tab(const QString& tabName) {
    const QString clean = tabName.trimmed().toLower();
    if (clean == QStringLiteral("logs")) {
        show_log_viewer_window();
        return;
    }
    if (clean == QStringLiteral("about")) {
        show_about_window();
        return;
    }
    show_main_window();
    if (QWindow* mainWin = find_main_window()) {
        if (!clean.isEmpty()) {
            mainWin->setProperty("activeTab", clean);
        }
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

        if (et == QEvent::Show) {
            if (auto* ovWin = qobject_cast<QWindow*>(watched)) {
                if (ovWin->objectName() == QStringLiteral("librelinkupOverlayWindow")) {
                    auto& st = OverlayPlacementState::instance();
                    if (st.hasPlacement && !st.dragging) {
                        st.applyToWindow(ovWin);
                    }
                }
            }
        }

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

inline bool librelinkup_is_main_window_shown() {
    return librelinkup::is_main_window_shown();
}

inline void librelinkup_show_main_window() {
    librelinkup::show_main_window();
}

inline void librelinkup_show_main_window_tab(const QString& tab_name) {
    librelinkup::show_main_window_tab(tab_name);
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
    for (int delayMs : {40, 140}) {
        QTimer::singleShot(delayMs, []() {
            if (QWindow* ovWin = librelinkup::find_overlay_window()) {
                auto& st = librelinkup::OverlayPlacementState::instance();
                if (!st.dragging) {
                    st.applyToWindow(ovWin, true);
                }
            }
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
