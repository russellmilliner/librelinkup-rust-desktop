#pragma once

// Umbrella C++ header registered with CxxQtBuilder.
// Holds NotificationActionWatcher (which requires Qt moc processing) and
// includes the modular C++ subsystems in src/cpp/.

#include <QObject>
#include <QString>

namespace librelinkup {

class NotificationActionWatcher : public QObject {
    Q_OBJECT
public:
    explicit NotificationActionWatcher(QObject* parent = nullptr) : QObject(parent) {}

public Q_SLOTS:
    void onActionInvoked(uint id, const QString& actionKey);
    void onNotificationClosed(uint id, uint reason);
    void onKWinOverlayGeometry(const QString& screenName, int globalX, int globalY, int width, int height);
    void onKWinMainWindowGeometry(const QString& screenName, int globalX, int globalY, int width, int height);
    QString getDesiredOverlaySync();
    QString getDesiredMainSync();
};

} // namespace librelinkup

#include "icon_renderer.h"
#include "window_manager.h"
#include "tray_manager.h"
#include "audio_portal.h"
#include "ui_tests.h"
