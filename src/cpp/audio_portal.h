#pragma once

#include <QApplication>
#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QFile>
#include <QGuiApplication>
#include <QIcon>
#include <QList>
#include <QLoggingCategory>
#include <QObject>
#include <QProcess>
#include <QQmlApplicationEngine>
#include <QRandomGenerator>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <QWindow>
#include <cstdint>
#include <cstdlib>

#include "icon_renderer.h"
#include "tray_manager.h"
#include "window_manager.h"

namespace librelinkup {
// Each command is {program, args...}; the sound file path is appended as the last argument.
using SoundCommand = QStringList;

// Tries each player command in order; advances to the next one if the player fails to
// start or exits with an error (e.g. no PipeWire/PulseAudio socket), finally beeping.
inline void play_sound_with_fallback(const QString& path, QList<SoundCommand> commands) {
    if (commands.isEmpty()) {
        QApplication::beep();
        return;
    }
    SoundCommand cmd = commands.takeFirst();
    const QString program = cmd.takeFirst();
    cmd.append(path);
    auto* proc = new QProcess(QCoreApplication::instance());
    QObject::connect(proc, &QProcess::errorOccurred, proc,
        [proc, path, commands](QProcess::ProcessError err) {
            if (err == QProcess::FailedToStart) {
                proc->deleteLater();
                play_sound_with_fallback(path, commands);
            }
        });
    QObject::connect(proc, &QProcess::finished, proc,
        [proc, path, commands](int exitCode, QProcess::ExitStatus status) {
            proc->deleteLater();
            if (status != QProcess::NormalExit || exitCode != 0) {
                play_sound_with_fallback(path, commands);
            }
        });
    proc->start(program, cmd);
}

// Alert playback commands. Each uses its own stream identity and an explicit full stream
// volume: PipeWire/WirePlumber restore saved per-application volumes, so playing as the
// generic "paplay" client would inherit whatever volume any other paplay user last left
// (including 0%, which made alerts silent). The device/master volume still applies.
inline QList<SoundCommand> alert_sound_commands() {
    const QString appName = QStringLiteral("LibreLinkUp Glucose Alert");
    return {
        // paplay first: Flatpak's --socket=pulseaudio exposes only the PulseAudio socket
        // (served by pipewire-pulse on modern hosts); pw-play needs the native PipeWire socket.
        {QStringLiteral("paplay"),
         QStringLiteral("--client-name=%1").arg(appName),
         QStringLiteral("--stream-name=Glucose alert"),
         QStringLiteral("--property=application.name=%1").arg(appName),
         QStringLiteral("--property=application.id=io.github.russellmilliner.LibreLinkUpDesktop"),
         QStringLiteral("--property=media.role=alarm"),
         QStringLiteral("--volume=65536")},
        {QStringLiteral("pw-play"),
         QStringLiteral("--volume=1.0"),
         QStringLiteral("--properties={ application.name = \"%1\" media.role = \"alarm\" }").arg(appName)},
    };
}
} // namespace librelinkup

inline void librelinkup_play_alert_sound(const QString& custom_sound_path) {
    const QString clean = custom_sound_path.trimmed();
    if (!clean.isEmpty() && QFile::exists(clean)) {
        librelinkup::play_sound_with_fallback(clean, librelinkup::alert_sound_commands());
        return;
    }
    QApplication::beep();
}

// Requests (or revokes) login autostart through the XDG Background portal
// (org.freedesktop.portal.Background.RequestBackground). Used inside Flatpak, where the
// sandbox must not write ~/.config/autostart directly; the portal (xdg-desktop-portal-kde
// on Plasma) creates/removes the host autostart entry on the app's behalf.
// Returns false if the session bus is unavailable. The outcome is logged via qInfo/qWarning.
inline bool librelinkup_request_background_autostart(bool enable, bool start_minimized) {
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        qWarning("librelinkup: session bus unavailable; cannot request autostart");
        return false;
    }
    QStringList commandline{QStringLiteral("librelinkup-kde")};
    if (start_minimized) {
        commandline << QStringLiteral("--minimized");
    }
    QVariantMap options;
    options.insert(QStringLiteral("handle_token"),
        QStringLiteral("librelinkup%1").arg(QRandomGenerator::global()->generate()));
    options.insert(QStringLiteral("reason"),
        QStringLiteral("Keep monitoring blood glucose and alerting after you log in."));
    options.insert(QStringLiteral("autostart"), enable);
    options.insert(QStringLiteral("commandline"), commandline);
    options.insert(QStringLiteral("dbus-activatable"), false);

    QDBusMessage msg = QDBusMessage::createMethodCall(
        QStringLiteral("org.freedesktop.portal.Desktop"),
        QStringLiteral("/org/freedesktop/portal/desktop"),
        QStringLiteral("org.freedesktop.portal.Background"),
        QStringLiteral("RequestBackground"));
    msg << QString() << options; // parent_window: none

    auto* watcher = new QDBusPendingCallWatcher(bus.asyncCall(msg), QCoreApplication::instance());
    QObject::connect(watcher, &QDBusPendingCallWatcher::finished, watcher,
        [enable](QDBusPendingCallWatcher* w) {
            QDBusPendingReply<QDBusObjectPath> reply = *w;
            if (reply.isError()) {
                qWarning("librelinkup: Background portal request failed: %s",
                    qPrintable(reply.error().message()));
            } else {
                qInfo("librelinkup: Background portal autostart=%s requested (%s)",
                    enable ? "true" : "false", qPrintable(reply.value().path()));
            }
            w->deleteLater();
        });
    return true;
}

inline void librelinkup_quit_app() {
    QCoreApplication::quit();
}

inline std::int32_t librelinkup_run_app(bool start_minimized) {
    QLoggingCategory::setFilterRules(QStringLiteral("union.*=false\nqt.core.qobject.connect=false"));
    if (!std::getenv("QT_QUICK_CONTROLS_STYLE")) {
        qputenv("QT_QUICK_CONTROLS_STYLE", "org.kde.breeze");
    }
    // Use XCB (XWayland/X11) by default when DISPLAY is available so KDE Plasma KWin
    // allows saving/restoring exact multi-monitor overlay coordinates and Qt::Tool taskbar skipping.
    if (std::getenv("DISPLAY") && !std::getenv("LIBRELINKUP_USE_WAYLAND") && !std::getenv("QT_QPA_PLATFORM")) {
        qputenv("QT_QPA_PLATFORM", "xcb");
    }
    if (start_minimized) {
        qputenv("LIBRELINKUP_START_MINIMIZED", "1");
    }

    static int argc = 1;
    static char arg0[] = "librelinkup-kde";
    static char* argv[] = {arg0, nullptr};

    QApplication app(argc, argv);
    librelinkup::install_wheel_scroll_filter();
    QGuiApplication::setApplicationName(QStringLiteral("LibreLinkUp Rust Desktop"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("LibreLinkUp Rust Desktop"));
    QGuiApplication::setDesktopFileName(QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop"));
    QGuiApplication::setQuitOnLastWindowClosed(false);

#ifndef QT_NO_SESSIONMANAGER
    if (app.isSessionRestored()) {
        start_minimized = true;
        qputenv("LIBRELINKUP_START_MINIMIZED", "1");
    }
#endif

    QIcon initialIcon = librelinkup::build_multisize_svg_icon(
        QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
    );
    QGuiApplication::setWindowIcon(initialIcon);

    QQmlApplicationEngine engine;
    engine.load(QUrl(QStringLiteral("qrc:/qt/qml/io/github/librelinkup/src/qml/Main.qml")));

    if (engine.rootObjects().isEmpty()) {
        return 1;
    }

    QWindow* win = qobject_cast<QWindow*>(engine.rootObjects().first());
    if (win) {
        librelinkup::main_window_ref() = win;
        librelinkup::ensure_window_hooks(win);
        win->setIcon(initialIcon);

        QObject* settingsCtrl = win->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"));
        if (settingsCtrl) {
            const bool ovVis = settingsCtrl->property("overlayVisible").toBool();
            const bool ovOnly = settingsCtrl->property("overlayOnlyMode").toBool();
            const QString ovScreen = settingsCtrl->property("overlayScreenName").toString();
            const int ovX = settingsCtrl->property("overlayX").toInt();
            const int ovY = settingsCtrl->property("overlayY").toInt();
            const int ovW = settingsCtrl->property("overlayWidth").toInt();
            const int ovH = settingsCtrl->property("overlayHeight").toInt();
            librelinkup::restore_overlay_placement(ovScreen, ovX, ovY, ovW, ovH);

            const QString winScreen = settingsCtrl->property("windowScreenName").toString();
            const int winX = settingsCtrl->property("windowX").toInt();
            const int winY = settingsCtrl->property("windowY").toInt();
            const int winW = settingsCtrl->property("windowWidth").toInt();
            const int winH = settingsCtrl->property("windowHeight").toInt();
            librelinkup::restore_main_window_placement(winScreen, winX, winY, winW, winH);

            if (ovVis && ovOnly) {
                start_minimized = true;
            }
        }

        if (start_minimized) {
            win->setVisible(false);
            win->hide();
            QTimer::singleShot(0, win, [win]() {
                if (win) {
                    win->setVisible(false);
                    win->hide();
                    librelinkup::update_tray_window_actions();
                }
            });
            QTimer::singleShot(120, win, [win]() {
                if (win) {
                    win->setVisible(false);
                    win->hide();
                    librelinkup::update_tray_window_actions();
                }
            });
        } else {
            librelinkup::MainWindowPlacementState::instance().applyToWindow(win, false);
            win->show();
            librelinkup::MainWindowPlacementState::instance().applyToWindow(win, true);
            win->raise();
            win->requestActivate();
            librelinkup::update_tray_window_actions();
        }
    }

    return app.exec();
}
