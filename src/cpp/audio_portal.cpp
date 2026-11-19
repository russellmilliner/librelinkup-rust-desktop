#include "audio_portal.h"

#include <QApplication>
#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QFileInfo>
#include <QGuiApplication>
#include <QIcon>
#include <QList>
#include <QLoggingCategory>
#include <QMetaObject>
#include <QObject>
#include <QPointer>
#include <QProcess>
#include <QQmlApplicationEngine>
#include <QRandomGenerator>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <QWindow>
#include <cstdlib>

#include "icon_renderer.h"
#include "tray_manager.h"
#include "window_manager.h"

namespace librelinkup {
// Each command is {program, args...}; the sound file path is appended as the last argument.
using SoundCommand = QStringList;

static QPointer<QProcess>& active_audio_process_ref() {
    static QPointer<QProcess> s_proc;
    return s_proc;
}

// Tries each player command in order; advances to the next one if the player fails to
// start or exits with an error (e.g. no PipeWire/PulseAudio socket), finally beeping.
static void play_sound_with_fallback(const QString& path, QList<SoundCommand> commands) {
    if (QCoreApplication::closingDown()) {
        return;
    }
    if (commands.isEmpty()) {
        QApplication::beep();
        return;
    }
    const QFileInfo fi(path);
    const QString canonicalPath = fi.canonicalFilePath();
    if (canonicalPath.isEmpty() || !fi.exists() || !fi.isFile()) {
        QApplication::beep();
        return;
    }
    if (QProcess* prev = active_audio_process_ref().data()) {
        active_audio_process_ref().clear();
        prev->disconnect();
        if (prev->state() != QProcess::NotRunning) {
            prev->kill();
        }
        prev->deleteLater();
    }
    SoundCommand cmd = commands.takeFirst();
    const QString program = cmd.takeFirst();
    cmd.append(QStringLiteral("--"));
    cmd.append(canonicalPath);
    auto* proc = new QProcess(QCoreApplication::instance());
    active_audio_process_ref() = proc;
    const QPointer<QProcess> procPtr(proc);
    QTimer::singleShot(15000, proc, [procPtr]() {
        if (procPtr && procPtr->state() != QProcess::NotRunning) {
            procPtr->kill();
        }
    });
    QObject::connect(proc, &QProcess::errorOccurred, proc,
        [procPtr, canonicalPath, commands](QProcess::ProcessError err) {
            if (err == QProcess::FailedToStart) {
                if (active_audio_process_ref().data() == procPtr.data()) {
                    active_audio_process_ref().clear();
                }
                if (procPtr) {
                    procPtr->disconnect();
                    procPtr->deleteLater();
                }
                play_sound_with_fallback(canonicalPath, commands);
            }
        });
    QObject::connect(proc, &QProcess::finished, proc,
        [procPtr, canonicalPath, commands](int exitCode, QProcess::ExitStatus status) {
            const bool isCurrent = (active_audio_process_ref().data() == procPtr.data());
            if (isCurrent) {
                active_audio_process_ref().clear();
            }
            if (procPtr) {
                procPtr->disconnect();
                procPtr->deleteLater();
            }
            if (isCurrent && (status != QProcess::NormalExit || exitCode != 0)) {
                play_sound_with_fallback(canonicalPath, commands);
            }
        });
    proc->start(program, cmd);
}

// Alert playback commands. Each uses its own stream identity and an explicit full stream
// volume: PipeWire/WirePlumber restore saved per-application volumes, so playing as the
// generic "paplay" client would inherit whatever volume any other paplay user last left
// (including 0%, which made alerts silent). The device/master volume still applies.
static QList<SoundCommand> alert_sound_commands() {
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

void librelinkup_play_alert_sound(const QString& custom_sound_path) {
    const QString clean = custom_sound_path.trimmed();
    if (!clean.isEmpty()) {
        const QFileInfo fi(clean);
        const QString canonical = fi.canonicalFilePath();
        if (!canonical.isEmpty() && fi.exists() && fi.isFile()) {
            librelinkup::play_sound_with_fallback(canonical, librelinkup::alert_sound_commands());
            return;
        }
    }
    QApplication::beep();
}

// Requests (or revokes) login autostart through the XDG Background portal
// (org.freedesktop.portal.Background.RequestBackground). Used inside Flatpak, where the
// sandbox must not write ~/.config/autostart directly; the portal (xdg-desktop-portal-kde
// on Plasma) creates/removes the host autostart entry on the app's behalf.
// Returns false if the session bus is unavailable. The outcome is logged via qInfo/qWarning.
bool librelinkup_request_background_autostart(bool enable, bool start_minimized) {
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

bool librelinkup_try_dispatch_cli_action(const QString& action_id) {
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return false;
    }
    QDBusMessage msg = QDBusMessage::createMethodCall(
        QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop"),
        QStringLiteral("/LibreLinkUpKWin"),
        QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.KWinBridge"),
        QStringLiteral("invokeGlobalAction")
    );
    msg << action_id;
    const QDBusMessage reply = bus.call(msg, QDBus::Block, 2000);
    return reply.type() != QDBusMessage::ErrorMessage;
}

void librelinkup_quit_app() {
    QCoreApplication::quit();
}

std::int32_t librelinkup_run_app(bool start_minimized, const QString& initial_action) {
    QLoggingCategory::setFilterRules(QStringLiteral("union.*=false\nqt.core.qobject.connect=false"));
    if (!std::getenv("QT_QUICK_CONTROLS_STYLE")) {
        qputenv("QT_QUICK_CONTROLS_STYLE", "org.kde.breeze");
    }
    // Use native Wayland by default when WAYLAND_DISPLAY is present; KWin D-Bus scripting
    // (org.kde.KWin /Scripting) and QWindow::startSystemMove() handle exact multi-monitor
    // coordinates, frameless overlay borders, taskbar skipping, and virtual desktop pinning.
    if (!std::getenv("QT_QPA_PLATFORM")) {
        if (std::getenv("WAYLAND_DISPLAY") && !std::getenv("LIBRELINKUP_USE_X11")) {
            qputenv("QT_QPA_PLATFORM", "wayland");
        } else if (std::getenv("DISPLAY")) {
            qputenv("QT_QPA_PLATFORM", "xcb");
        }
    }

    static int argc = 1;
    static char arg0[] = "librelinkup-kde";
    static char* argv[] = {arg0, nullptr};

    QApplication app(argc, argv);
    librelinkup::install_wheel_scroll_filter();
    librelinkup::ensure_kwin_dbus_bridge();
    QGuiApplication::setApplicationName(QStringLiteral("LibreLinkUp Rust Desktop"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("LibreLinkUp Rust Desktop"));
    QGuiApplication::setDesktopFileName(QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop"));
    QGuiApplication::setQuitOnLastWindowClosed(false);

#ifndef QT_NO_SESSIONMANAGER
    if (app.isSessionRestored()) {
        start_minimized = true;
    }
#endif

    QObject::connect(&app, &QCoreApplication::aboutToQuit, &app, []() {
        if (QWindow* win = librelinkup::main_window_ref().data()) {
            if (QObject* ctrl = win->findChild<QObject*>(QStringLiteral("librelinkupAppController"))) {
                QMetaObject::invokeMethod(ctrl, "flushHistoryOnShutdown", Qt::DirectConnection);
            }
        }
    });

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

    const QString cleanInitialAction = initial_action.trimmed();
    if (!cleanInitialAction.isEmpty()) {
        QTimer::singleShot(350, &app, [cleanInitialAction]() {
            librelinkup::TrayManager::instance().handleGlobalShortcutAction(cleanInitialAction);
        });
    }

    return app.exec();
}
