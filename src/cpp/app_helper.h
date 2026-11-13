#pragma once

// Umbrella C++ header registered with CxxQtBuilder.
// Holds NotificationActionWatcher (which requires Qt moc processing) and
// includes the modular C++ subsystems in src/cpp/.

#include <QObject>
#include <QString>

namespace librelinkup {

class NotificationActionWatcher : public QObject {
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "io.github.russellmilliner.LibreLinkUpDesktop.KWinBridge")
public:
    explicit NotificationActionWatcher(QObject* parent = nullptr) : QObject(parent) {}

public Q_SLOTS:
    void onActionInvoked(uint id, const QString& actionKey);
    void onNotificationClosed(uint id, uint reason);
    void onGlobalShortcutPressed(const QString& componentUnique, const QString& shortcutUnique, qlonglong timestamp);
    Q_SCRIPTABLE void invokeGlobalAction(const QString& actionId);
    Q_SCRIPTABLE void onKWinOverlayGeometry(const QString& screenName, int globalX, int globalY, int width, int height);
    Q_SCRIPTABLE void onKWinMainWindowGeometry(const QString& screenName, int globalX, int globalY, int width, int height);
    Q_SCRIPTABLE QString getDesiredOverlaySync();
    Q_SCRIPTABLE QString getDesiredMainSync();
};

} // namespace librelinkup

#include "icon_renderer.h"
#include "window_manager.h"
#include "tray_manager.h"
#include "audio_portal.h"
#include "ui_tests.h"
