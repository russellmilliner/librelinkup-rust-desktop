#pragma once

#include <QHash>
#include <QIcon>
#include <QObject>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QtGlobal>

class KStatusNotifierItem;
class QAction;
class QDBusConnection;
class QMenu;
class QSystemTrayIcon;

namespace librelinkup {

class NotificationActionWatcher;

class TrayManager {
public:
    static TrayManager& instance();

    bool isTrayIconVisible() const;
    int statusNotifierStatus() const;
    int overlayOnlyModeInvokeCount() const;
    void popupMenuAtCursor();

    static QString resolveNotificationEventId(
        const QString& title,
        bool critical,
        bool withSnoozeActions,
        const QString& medicationKey = QString()
    );

    bool isKdeNotificationEventPopupEnabled(
        const QString& eventId,
        const QString& configDirOverride = QString()
    ) const;

    void showNotification(
        const QString& title,
        const QString& body,
        bool critical,
        bool withSnoozeActions = false
    );

    void showMedicationNotification(
        const QString& medicationId,
        const QString& title,
        const QString& body
    );

    void handleNotificationAction(uint id, const QString& actionKey);
    void handleNotificationClosed(uint id);
    void updateWindowActionsVisibility();

    void sync(
        QObject* controller,
        bool useLightOnDarkIcon,
        bool showGlucoseInIcon,
        bool showGlucoseInTooltip,
        bool autostartEnabled,
        bool overlayVisible,
        bool overlayOnAllDesktops,
        bool overlayOnlyMode,
        bool onlyRelevantOutOfRange,
        bool trayIsRelevant,
        bool trayNeedsAttention,
        const QString& appThemeMode,
        const QString& badgeText,
        const QString& trendSymbol,
        const QString& badgeColorHex,
        const QString& statusHeader,
        const QString& rangeHeader,
        const QString& tooltipText
    );

    static QByteArray canonicalKdeNotifyRcBytes();
    static void syncSecondaryControllers(Qt::ConnectionType connType);
    bool handleGlobalShortcutAction(const QString& actionId);

private:
    TrayManager() = default;

    QObject* settingsController() const;
    void ensureInitialized();
    void ensureKdeNotificationConfigInstalled();
    void ensureNotificationWatcherConnected(QDBusConnection& bus);
    void registerKdeGlobalShortcuts();
    bool sendDesktopNotificationWithActions(
        const QString& title,
        const QString& body,
        bool critical,
        const QStringList& actions,
        bool replaceLastAlert,
        const QString& medicationKey = QString(),
        const QString& eventId = QString()
    );

    bool m_initialized = false;
    bool m_overlayVisible = false;
    bool m_overlayOnlyMode = false;
    int m_overlayOnlyModeInvokeCount = 0;
    QPointer<QObject> m_controller;
    QPointer<KStatusNotifierItem> m_sni;
    QPointer<QSystemTrayIcon> m_tray;
    QIcon m_currentTrayIcon;
    QIcon m_cachedDarkBaseIcon;
    QIcon m_cachedLightBaseIcon;
    QPointer<QMenu> m_menu;
    QPointer<QAction> m_statusAction;
    QPointer<QAction> m_rangeAction;
    QPointer<QAction> m_showInIconAction;
    QPointer<QAction> m_showInTooltipAction;
    QPointer<QAction> m_onlyRelevantOutOfRangeAction;
    QPointer<QAction> m_overlayAction;
    QPointer<QAction> m_overlayAllDesktopsAction;
    QPointer<QAction> m_autostartAction;
    QPointer<QAction> m_themeSystemAction;
    QPointer<QAction> m_themeDarkAction;
    QPointer<QAction> m_themeLightAction;
    QPointer<QAction> m_showAction;
    QPointer<QAction> m_hideAction;
    QString m_lastIconKey;
    QString m_lastTooltipText;
    QString m_lastStatusHeader;
    QString m_lastRangeHeader;
    QPointer<NotificationActionWatcher> m_notifWatcher;
    QSet<uint> m_activeAlertNotificationIds;
    QHash<QString, uint> m_activeMedicationNotificationIds;
    uint m_lastAlertNotificationId = 0;
    QString m_lastShortcutActionId;
    qint64 m_lastShortcutEpochMs = 0;
};

void update_tray_window_actions();

} // namespace librelinkup

// Two template parameters: the payload struct is declared by the generated cxx bridge header,
// which is processed *after* this header is included, so the type must stay dependent here.
template <typename ControllerT, typename PayloadT>
inline void librelinkup_sync_tray(ControllerT& controller, PayloadT payload) {
    const bool useLightOnDarkIcon = (payload.flags & 0x01u) != 0u;
    const bool showGlucoseInIcon = (payload.flags & 0x02u) != 0u;
    const bool showGlucoseInTooltip = (payload.flags & 0x04u) != 0u;
    const bool autostartEnabled = (payload.flags & 0x08u) != 0u;
    const bool overlayVisible = (payload.flags & 0x10u) != 0u;
    const bool overlayOnAllDesktops = (payload.flags & 0x20u) != 0u;
    const bool overlayOnlyMode = (payload.flags & 0x40u) != 0u;
    const bool onlyRelevantOutOfRange = (payload.flags & 0x80u) != 0u;
    const bool trayIsRelevant = (payload.flags & 0x100u) != 0u;
    const bool trayNeedsAttention = (payload.flags & 0x200u) != 0u;

    librelinkup::TrayManager::instance().sync(
        &controller,
        useLightOnDarkIcon,
        showGlucoseInIcon,
        showGlucoseInTooltip,
        autostartEnabled,
        overlayVisible,
        overlayOnAllDesktops,
        overlayOnlyMode,
        onlyRelevantOutOfRange,
        trayIsRelevant,
        trayNeedsAttention,
        payload.theme_mode,
        payload.badge_text,
        payload.trend_symbol,
        payload.badge_color_hex,
        payload.status_header,
        payload.range_header,
        payload.tooltip_text
    );
}

void librelinkup_apply_app_theme(const QString& themeMode);
void librelinkup_popup_tray_menu();
void librelinkup_show_notification(
    const QString& title,
    const QString& body,
    bool critical,
    bool with_snooze_actions
);
void librelinkup_show_medication_notification(
    const QString& medication_id,
    const QString& title,
    const QString& body
);
