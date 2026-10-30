#pragma once

#include <KColorScheme>
#include <KColorSchemeManager>
#include <KIconLoader>
#include <KSharedConfig>
#include <QAbstractItemModel>
#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QCoreApplication>
#include <QCursor>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDesktopServices>
#include <QGuiApplication>
#include <QIcon>
#include <QList>
#include <QMenu>
#include <QMetaObject>
#include <QModelIndex>
#include <QObject>
#include <QPair>
#include <QPalette>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QSystemTrayIcon>
#include <QUrl>
#include <QVariant>
#include <QWindow>
#include <cstdint>

#include "icon_renderer.h"
#include "window_manager.h"

namespace librelinkup {

class TrayManager {
public:
    static TrayManager& instance() {
        static TrayManager inst;
        return inst;
    }

    bool isTrayIconVisible() const {
        return m_tray && m_tray->isVisible();
    }

    void popupMenuAtCursor() {
        ensureInitialized();
        if (m_menu) {
            updateWindowActionsVisibility();
            m_menu->popup(QCursor::pos());
        }
    }

    // For alert notifications (withSnoozeActions = true), uses org.freedesktop.Notifications
    // directly so interactive snooze buttons ("Snooze 30 min", "Snooze 1 hr", "Snooze 2 hr")
    // are attached, falling back to QSystemTrayIcon::showMessage when D-Bus is unavailable.
    void showNotification(
        const QString& title,
        const QString& body,
        bool critical,
        bool withSnoozeActions = false
    ) {
        ensureInitialized();
        const QStringList snoozeActions{
            QStringLiteral("default"), QStringLiteral("Open"),
            QStringLiteral("snooze_30"), QStringLiteral("Snooze 30 min"),
            QStringLiteral("snooze_60"), QStringLiteral("Snooze 1 hr"),
            QStringLiteral("snooze_120"), QStringLiteral("Snooze 2 hr"),
        };
        if (withSnoozeActions &&
            sendDesktopNotificationWithActions(title, body, critical, snoozeActions, true)) {
            return;
        }
        if (!m_tray || !QSystemTrayIcon::supportsMessages()) {
            return;
        }
        m_tray->showMessage(
            title,
            body,
            critical ? QSystemTrayIcon::Critical : QSystemTrayIcon::Information,
            15000
        );
    }

    // Sends a desktop medication due reminder with interactive "✓ Taken" and "⏭ Skipped" buttons.
    void showMedicationNotification(
        const QString& medicationId,
        const QString& title,
        const QString& body
    ) {
        ensureInitialized();
        const QStringList medActions{
            QStringLiteral("default"), QStringLiteral("Open"),
            QStringLiteral("med_taken:%1").arg(medicationId), QStringLiteral("✓ Taken"),
            QStringLiteral("med_skip:%1").arg(medicationId), QStringLiteral("⏭ Skipped"),
        };
        if (sendDesktopNotificationWithActions(title, body, false, medActions, false)) {
            return;
        }
        if (!m_tray || !QSystemTrayIcon::supportsMessages()) {
            return;
        }
        m_tray->showMessage(
            title,
            body,
            QSystemTrayIcon::Information,
            15000
        );
    }

    void handleNotificationAction(uint id, const QString& actionKey) {
        if (!m_activeAlertNotificationIds.contains(id) && id != m_lastAlertNotificationId) {
            return;
        }
        if (m_controller) {
            QMetaObject::invokeMethod(
                m_controller,
                "handleNotificationAction",
                Qt::QueuedConnection,
                Q_ARG(QString, actionKey)
            );
        }
        m_activeAlertNotificationIds.remove(id);
        if (m_lastAlertNotificationId == id) {
            m_lastAlertNotificationId = 0;
        }
        QDBusConnection bus = QDBusConnection::sessionBus();
        if (bus.isConnected() && id > 0) {
            QDBusMessage closeMsg = QDBusMessage::createMethodCall(
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("/org/freedesktop/Notifications"),
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("CloseNotification")
            );
            closeMsg << id;
            bus.asyncCall(closeMsg);
        }
    }

    void handleNotificationClosed(uint id) {
        m_activeAlertNotificationIds.remove(id);
        if (m_lastAlertNotificationId == id) {
            m_lastAlertNotificationId = 0;
        }
    }

    void updateWindowActionsVisibility() {
        const bool shown = is_main_window_shown();
        if (m_showAction) {
            m_showAction->setVisible(!shown);
        }
        if (m_hideAction) {
            m_hideAction->setVisible(shown);
        }

        QWindow* ovWin = find_overlay_window();
        const bool ovShown = m_overlayVisible || (ovWin && ovWin->isVisible());
        const bool onlyOverlayShowing = ovShown && !shown;

        if (m_controller) {
            const bool curOnly = m_controller->property("overlayOnlyMode").toBool();
            if (curOnly != onlyOverlayShowing) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateOverlayOnlyMode",
                    Qt::QueuedConnection,
                    Q_ARG(bool, onlyOverlayShowing)
                );
            }
        }

        if (m_tray && !m_tray->isVisible()) {
            m_tray->show();
        }
    }

    void sync(
        QObject* controller,
        bool useLightOnDarkIcon,
        bool showGlucoseInIcon,
        bool showGlucoseInTooltip,
        bool autostartEnabled,
        bool overlayVisible,
        bool overlayOnAllDesktops,
        const QString& appThemeMode,
        const QString& badgeText,
        const QString& trendSymbol,
        const QString& badgeColorHex,
        const QString& statusHeader,
        const QString& rangeHeader,
        const QString& tooltipText
    ) {
        m_controller = controller;
        m_overlayVisible = overlayVisible;
        ensureInitialized();
        if (!m_tray) {
            return;
        }

        const QString cacheKey = QStringLiteral("%1|%2|%3|%4|%5")
            .arg(useLightOnDarkIcon ? 1 : 0)
            .arg(showGlucoseInIcon ? 1 : 0)
            .arg(badgeText, trendSymbol, badgeColorHex);

        if (m_lastIconKey != cacheKey) {
            m_lastIconKey = cacheKey;
            QIcon baseAppIcon = build_multisize_svg_icon(
                useLightOnDarkIcon
                    ? QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
                    : QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-light.svg")
            );
            QGuiApplication::setWindowIcon(baseAppIcon);
            if (QWindow* win = find_main_window()) {
                win->setIcon(baseAppIcon);
            }
            if (showGlucoseInIcon && !badgeText.trimmed().isEmpty() && badgeText != QStringLiteral("--")) {
                QIcon badgeIcon = build_glucose_badge_icon(badgeText, trendSymbol, badgeColorHex);
                m_tray->setIcon(badgeIcon);
            } else {
                m_tray->setIcon(baseAppIcon);
            }
        }

        m_tray->setToolTip(tooltipText);

        if (m_statusAction) {
            m_statusAction->setText(statusHeader);
        }
        if (m_rangeAction) {
            m_rangeAction->setText(rangeHeader);
        }
        if (m_showInIconAction) {
            const bool old = m_showInIconAction->blockSignals(true);
            m_showInIconAction->setChecked(showGlucoseInIcon);
            m_showInIconAction->blockSignals(old);
        }
        if (m_showInTooltipAction) {
            const bool old = m_showInTooltipAction->blockSignals(true);
            m_showInTooltipAction->setChecked(showGlucoseInTooltip);
            m_showInTooltipAction->blockSignals(old);
        }
        if (m_autostartAction) {
            const bool old = m_autostartAction->blockSignals(true);
            m_autostartAction->setChecked(autostartEnabled);
            m_autostartAction->blockSignals(old);
        }
        if (m_overlayAction) {
            const bool old = m_overlayAction->blockSignals(true);
            m_overlayAction->setChecked(overlayVisible);
            m_overlayAction->blockSignals(old);
        }
        if (m_overlayAllDesktopsAction) {
            const bool old = m_overlayAllDesktopsAction->blockSignals(true);
            m_overlayAllDesktopsAction->setChecked(overlayOnAllDesktops);
            m_overlayAllDesktopsAction->blockSignals(old);
        }
        if (m_themeSystemAction && m_themeDarkAction && m_themeLightAction) {
            m_themeSystemAction->blockSignals(true);
            m_themeDarkAction->blockSignals(true);
            m_themeLightAction->blockSignals(true);
            m_themeSystemAction->setChecked(appThemeMode == QStringLiteral("system"));
            m_themeDarkAction->setChecked(appThemeMode == QStringLiteral("dark"));
            m_themeLightAction->setChecked(appThemeMode == QStringLiteral("light"));
            m_themeSystemAction->blockSignals(false);
            m_themeDarkAction->blockSignals(false);
            m_themeLightAction->blockSignals(false);
        }

        updateWindowActionsVisibility();
    }

private:
    TrayManager() = default;

    void ensureInitialized() {
        if (m_initialized || !QCoreApplication::instance()) {
            return;
        }
        m_initialized = true;

        QGuiApplication::setQuitOnLastWindowClosed(false);

        m_tray = new QSystemTrayIcon(QCoreApplication::instance());
        m_tray->setIcon(build_multisize_svg_icon(
            QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
        ));
        m_menu = new QMenu();

        QObject::connect(m_menu, &QMenu::aboutToShow, [this]() {
            updateWindowActionsVisibility();
        });

        m_statusAction = m_menu->addAction(QStringLiteral("Blood Sugar: --"));
        QObject::connect(m_statusAction, &QAction::triggered, []() {
            show_main_window();
        });

        m_rangeAction = m_menu->addAction(QStringLiteral("Target Range: 70–180 mg/dL"));
        m_rangeAction->setEnabled(false);

        m_menu->addSeparator();

        QAction* refreshAction = m_menu->addAction(
            QIcon::fromTheme(QStringLiteral("view-refresh")),
            QStringLiteral("Refresh Reading Now")
        );
        QObject::connect(refreshAction, &QAction::triggered, [this]() {
            if (m_controller) {
                QMetaObject::invokeMethod(m_controller, "refreshGlucoseNow", Qt::QueuedConnection);
            }
        });

        // Snooze non-urgent alerts (urgent low always alerts).
        QMenu* snoozeMenu = m_menu->addMenu(
            QIcon::fromTheme(QStringLiteral("notifications-disabled")),
            QStringLiteral("Snooze Alerts")
        );
        const QList<QPair<QString, int>> snoozeOptions = {
            {QStringLiteral("30 minutes"), 30},
            {QStringLiteral("1 hour"), 60},
            {QStringLiteral("2 hours"), 120},
            {QStringLiteral("Cancel snooze"), 0},
        };
        for (const auto& opt : snoozeOptions) {
            const int minutes = opt.second;
            if (minutes == 0) {
                snoozeMenu->addSeparator();
            }
            QAction* act = snoozeMenu->addAction(opt.first);
            QObject::connect(act, &QAction::triggered, [this, minutes]() {
                if (m_controller) {
                    QMetaObject::invokeMethod(
                        m_controller, "snoozeAlerts", Qt::QueuedConnection, Q_ARG(int, minutes));
                }
            });
        }

        m_showInIconAction = m_menu->addAction(QStringLiteral("Show Glucose Value in Tray Icon"));
        m_showInIconAction->setCheckable(true);
        QObject::connect(m_showInIconAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateShowGlucoseInTrayIcon",
                    Qt::QueuedConnection,
                    Q_ARG(bool, checked)
                );
            }
        });

        m_showInTooltipAction = m_menu->addAction(QStringLiteral("Show Glucose Info on Tray Hover"));
        m_showInTooltipAction->setCheckable(true);
        QObject::connect(m_showInTooltipAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateShowGlucoseInTrayTooltip",
                    Qt::QueuedConnection,
                    Q_ARG(bool, checked)
                );
            }
        });

        m_overlayAction = m_menu->addAction(QStringLiteral("Show Floating Desktop Overlay"));
        m_overlayAction->setCheckable(true);
        QObject::connect(m_overlayAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateOverlayVisible",
                    Qt::QueuedConnection,
                    Q_ARG(bool, checked)
                );
            }
        });

        m_overlayAllDesktopsAction = m_menu->addAction(QStringLiteral("Show Overlay on All Desktops"));
        m_overlayAllDesktopsAction->setCheckable(true);
        QObject::connect(m_overlayAllDesktopsAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateOverlayOnAllDesktops",
                    Qt::QueuedConnection,
                    Q_ARG(bool, checked)
                );
            }
        });

        m_autostartAction = m_menu->addAction(QStringLiteral("Start Minimized on Login"));
        m_autostartAction->setCheckable(true);
        QObject::connect(m_autostartAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateAutostartEnabled",
                    Qt::QueuedConnection,
                    Q_ARG(bool, checked)
                );
            }
        });

        QMenu* themeMenu = m_menu->addMenu(QStringLiteral("Color Theme"));
        QActionGroup* themeGroup = new QActionGroup(themeMenu);
        themeGroup->setExclusive(true);

        m_themeSystemAction = themeMenu->addAction(QStringLiteral("System (Follow KDE Plasma)"));
        m_themeSystemAction->setCheckable(true);
        themeGroup->addAction(m_themeSystemAction);
        QObject::connect(m_themeSystemAction, &QAction::triggered, [this]() {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateThemeMode",
                    Qt::QueuedConnection,
                    Q_ARG(QString, QStringLiteral("system"))
                );
            }
        });

        m_themeDarkAction = themeMenu->addAction(QStringLiteral("Dark Mode (Breeze Dark)"));
        m_themeDarkAction->setCheckable(true);
        themeGroup->addAction(m_themeDarkAction);
        QObject::connect(m_themeDarkAction, &QAction::triggered, [this]() {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateThemeMode",
                    Qt::QueuedConnection,
                    Q_ARG(QString, QStringLiteral("dark"))
                );
            }
        });

        m_themeLightAction = themeMenu->addAction(QStringLiteral("Light Mode (Breeze Light)"));
        m_themeLightAction->setCheckable(true);
        themeGroup->addAction(m_themeLightAction);
        QObject::connect(m_themeLightAction, &QAction::triggered, [this]() {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateThemeMode",
                    Qt::QueuedConnection,
                    Q_ARG(QString, QStringLiteral("light"))
                );
            }
        });

        m_menu->addSeparator();

        QAction* logViewerAction = m_menu->addAction(
            QIcon::fromTheme(QStringLiteral("utilities-log-viewer")),
            QStringLiteral("Open Log Viewer")
        );
        QObject::connect(logViewerAction, &QAction::triggered, []() {
            show_log_viewer_window();
        });

        QAction* checkUpdatesAction = m_menu->addAction(
            QIcon::fromTheme(QStringLiteral("system-software-update")),
            QStringLiteral("Check for Updates")
        );
        QObject::connect(checkUpdatesAction, &QAction::triggered, [this]() {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "checkForUpdates",
                    Qt::QueuedConnection
                );
            }
        });

        QAction* aboutAction = m_menu->addAction(
            QIcon::fromTheme(QStringLiteral("help-about")),
            QStringLiteral("About LibreLinkUp Rust Desktop...")
        );
        QObject::connect(aboutAction, &QAction::triggered, []() {
            show_about_window();
        });

        QAction* kofiAction = m_menu->addAction(
            QStringLiteral("☕ Buy Me a Coffee (Ko-fi)...")
        );
        QObject::connect(kofiAction, &QAction::triggered, []() {
            QDesktopServices::openUrl(QUrl(QStringLiteral("https://ko-fi.com/russellmilliner")));
        });

        m_showAction = m_menu->addAction(QStringLiteral("Show LibreLinkUp Rust Desktop"));
        QObject::connect(m_showAction, &QAction::triggered, []() {
            show_main_window();
        });

        m_hideAction = m_menu->addAction(QStringLiteral("Minimize to Tray"));
        QObject::connect(m_hideAction, &QAction::triggered, []() {
            hide_main_window_to_tray();
        });

        m_menu->addSeparator();

        QAction* quitAction = m_menu->addAction(
            QIcon::fromTheme(QStringLiteral("application-exit")),
            QStringLiteral("Quit LibreLinkUp Rust Desktop")
        );
        QObject::connect(quitAction, &QAction::triggered, []() {
            QCoreApplication::quit();
        });

        updateWindowActionsVisibility();
        m_tray->setContextMenu(m_menu);

        QObject::connect(
            m_tray,
            &QSystemTrayIcon::activated,
            [](QSystemTrayIcon::ActivationReason reason) {
                if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) {
                    if (is_main_window_shown()) {
                        hide_main_window_to_tray();
                    } else {
                        show_main_window();
                    }
                }
            }
        );
    }

    bool m_initialized = false;
    bool m_overlayVisible = false;
    QPointer<QObject> m_controller;
    QPointer<QSystemTrayIcon> m_tray;
    QMenu* m_menu = nullptr;
    QAction* m_statusAction = nullptr;
    QAction* m_rangeAction = nullptr;
    QAction* m_showInIconAction = nullptr;
    QAction* m_showInTooltipAction = nullptr;
    QAction* m_overlayAction = nullptr;
    QAction* m_overlayAllDesktopsAction = nullptr;
    QAction* m_autostartAction = nullptr;
    QAction* m_themeSystemAction = nullptr;
    QAction* m_themeDarkAction = nullptr;
    QAction* m_themeLightAction = nullptr;
    QAction* m_showAction = nullptr;
    QAction* m_hideAction = nullptr;
    QString m_lastIconKey;
    QPointer<NotificationActionWatcher> m_notifWatcher;
    QSet<uint> m_activeAlertNotificationIds;
    uint m_lastAlertNotificationId = 0;

    bool sendDesktopNotificationWithActions(
        const QString& title,
        const QString& body,
        bool critical,
        const QStringList& actions,
        bool replaceLastAlert
    ) {
        if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
            return false;
        }
        QDBusConnection bus = QDBusConnection::sessionBus();
        if (!bus.isConnected()) {
            return false;
        }
        if (!m_notifWatcher) {
            m_notifWatcher = new NotificationActionWatcher(QCoreApplication::instance());
            bus.connect(
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("/org/freedesktop/Notifications"),
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("ActionInvoked"),
                m_notifWatcher,
                SLOT(onActionInvoked(uint,QString))
            );
            bus.connect(
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("/org/freedesktop/Notifications"),
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("NotificationClosed"),
                m_notifWatcher,
                SLOT(onNotificationClosed(uint,uint))
            );
        }

        QVariantMap hints;
        hints.insert(
            QStringLiteral("urgency"),
            QVariant::fromValue<uchar>(critical ? 2u : 1u)
        );
        hints.insert(
            QStringLiteral("desktop-entry"),
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop")
        );

        QDBusMessage msg = QDBusMessage::createMethodCall(
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("/org/freedesktop/Notifications"),
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("Notify")
        );
        msg << QStringLiteral("LibreLinkUp Rust Desktop")
            << (replaceLastAlert ? m_lastAlertNotificationId : static_cast<uint>(0))
            << QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop")
            << title
            << body
            << actions
            << hints
            << static_cast<int>(15000);

        auto* watcher = new QDBusPendingCallWatcher(bus.asyncCall(msg), QCoreApplication::instance());
        QObject::connect(
            watcher,
            &QDBusPendingCallWatcher::finished,
            watcher,
            [this, title, body, critical, replaceLastAlert](QDBusPendingCallWatcher* w) {
                QDBusPendingReply<uint> reply = *w;
                if (reply.isError()) {
                    qWarning(
                        "librelinkup: org.freedesktop.Notifications.Notify failed (%s); falling back to tray message",
                        qPrintable(reply.error().message())
                    );
                    if (m_tray && QSystemTrayIcon::supportsMessages()) {
                        m_tray->showMessage(
                            title,
                            body,
                            critical ? QSystemTrayIcon::Critical : QSystemTrayIcon::Information,
                            15000
                        );
                    }
                } else {
                    const uint notifId = reply.value();
                    if (notifId > 0) {
                        if (replaceLastAlert) {
                            m_lastAlertNotificationId = notifId;
                        }
                        m_activeAlertNotificationIds.insert(notifId);
                    }
                }
                w->deleteLater();
            }
        );
        return true;
    }
};

inline void NotificationActionWatcher::onActionInvoked(uint id, const QString& actionKey) {
    TrayManager::instance().handleNotificationAction(id, actionKey);
}

inline void NotificationActionWatcher::onNotificationClosed(uint id, uint reason) {
    Q_UNUSED(reason);
    TrayManager::instance().handleNotificationClosed(id);
}

inline void update_tray_window_actions() {
    TrayManager::instance().updateWindowActionsVisibility();
}

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

    librelinkup::TrayManager::instance().sync(
        &controller,
        useLightOnDarkIcon,
        showGlucoseInIcon,
        showGlucoseInTooltip,
        autostartEnabled,
        overlayVisible,
        overlayOnAllDesktops,
        payload.theme_mode,
        payload.badge_text,
        payload.trend_symbol,
        payload.badge_color_hex,
        payload.status_header,
        payload.range_header,
        payload.tooltip_text
    );
}

inline void librelinkup_apply_app_theme(const QString& themeMode) {
    if (!QCoreApplication::instance()) {
        return;
    }
    KColorSchemeManager* mgr = KColorSchemeManager::instance();
    if (!mgr) {
        return;
    }
    mgr->setAutosaveChanges(false);
    QModelIndex targetIdx;
    if (themeMode == QStringLiteral("dark")) {
        targetIdx = mgr->indexForSchemeId(QStringLiteral("BreezeDark"));
        if (!targetIdx.isValid()) {
            targetIdx = mgr->indexForScheme(QStringLiteral("Breeze Dark"));
        }
    } else if (themeMode == QStringLiteral("light")) {
        targetIdx = mgr->indexForSchemeId(QStringLiteral("BreezeLight"));
        if (!targetIdx.isValid()) {
            targetIdx = mgr->indexForScheme(QStringLiteral("Breeze Light"));
        }
    }

    if (targetIdx.isValid()) {
        mgr->activateScheme(targetIdx);
        if (const QAbstractItemModel* model = mgr->model()) {
            const QString schemePath = model->data(targetIdx, Qt::UserRole).toString();
            if (!schemePath.isEmpty()) {
                const QPalette pal = KColorScheme::createApplicationPalette(
                    KSharedConfig::openConfig(schemePath)
                );
                QApplication::setPalette(pal);
                if (KIconLoader* loader = KIconLoader::global()) {
                    loader->setCustomPalette(pal);
                    KIconLoader::emitChange(KIconLoader::Desktop);
                }
            }
        }
    } else {
        mgr->activateScheme(QModelIndex());
        const QPalette sysPal = KColorScheme::createApplicationPalette(
            KSharedConfig::openConfig()
        );
        QApplication::setPalette(sysPal);
        if (KIconLoader* loader = KIconLoader::global()) {
            loader->resetPalette();
            KIconLoader::emitChange(KIconLoader::Desktop);
        }
    }
}

inline void librelinkup_popup_tray_menu() {
    librelinkup::TrayManager::instance().popupMenuAtCursor();
}

inline void librelinkup_show_notification(
    const QString& title,
    const QString& body,
    bool critical,
    bool with_snooze_actions
) {
    librelinkup::TrayManager::instance().showNotification(
        title,
        body,
        critical,
        with_snooze_actions
    );
}

inline void librelinkup_show_medication_notification(
    const QString& medication_id,
    const QString& title,
    const QString& body
) {
    librelinkup::TrayManager::instance().showMedicationNotification(
        medication_id,
        title,
        body
    );
}
