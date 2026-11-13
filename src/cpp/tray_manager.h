#pragma once

#include <KColorScheme>
#include <KColorSchemeManager>
#include <KConfigGroup>
#include <KIconLoader>
#include <KSharedConfig>
#include <KStatusNotifierItem>
#include <QAbstractItemModel>
#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QCoreApplication>
#include <QCursor>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QHash>
#include <QIcon>
#include <QList>
#include <QMenu>
#include <QMetaObject>
#include <QMetaType>
#include <QModelIndex>
#include <QObject>
#include <QPair>
#include <QPalette>
#include <QPointer>
#include <QSaveFile>
#include <QSet>
#include <QStandardPaths>
#include <QString>
#include <QStringList>
#include <QSystemTrayIcon>
#include <QThread>
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
        return m_sni != nullptr || (m_tray && m_tray->isVisible());
    }

    int statusNotifierStatus() const {
        return m_sni ? static_cast<int>(m_sni->status()) : 0;
    }

    int overlayOnlyModeInvokeCount() const {
        return m_overlayOnlyModeInvokeCount;
    }

    void popupMenuAtCursor() {
        ensureInitialized();
        if (m_menu) {
            updateWindowActionsVisibility();
            if (is_wayland_platform()) {
                m_menu->winId();
                if (QWindow* menuWin = m_menu->windowHandle()) {
                    if (QWindow* ovWin = find_overlay_window(); ovWin && ovWin->isVisible()) {
                        menuWin->setTransientParent(ovWin);
                    } else if (QWindow* mainWin = find_main_window(); mainWin && mainWin->isVisible()) {
                        menuWin->setTransientParent(mainWin);
                    }
                }
            }
            m_menu->popup(QCursor::pos());
        }
    }

    static QString resolveNotificationEventId(
        const QString& title,
        bool critical,
        bool withSnoozeActions,
        const QString& medicationKey = QString()
    ) {
        if (!medicationKey.trimmed().isEmpty()
            || title.contains(QStringLiteral("Medication"), Qt::CaseInsensitive)) {
            return QStringLiteral("medicationDue");
        }
        if (title.contains(QStringLiteral("Urgent Low"), Qt::CaseInsensitive)) {
            return QStringLiteral("urgentLowGlucose");
        }
        if (title.contains(QStringLiteral("Test"), Qt::CaseInsensitive)) {
            return QStringLiteral("alertTest");
        }
        if (title.contains(QStringLiteral("Low Glucose"), Qt::CaseInsensitive)) {
            return QStringLiteral("lowGlucose");
        }
        if (title.contains(QStringLiteral("High Glucose"), Qt::CaseInsensitive)) {
            return QStringLiteral("highGlucose");
        }
        if (title.contains(QStringLiteral("Stale"), Qt::CaseInsensitive)) {
            return QStringLiteral("staleReading");
        }
        if (title.contains(QStringLiteral("Sensor Expiring"), Qt::CaseInsensitive)) {
            return QStringLiteral("sensorExpiring");
        }
        if (title.contains(QStringLiteral("Update"), Qt::CaseInsensitive)
            || title.contains(QStringLiteral("Up to Date"), Qt::CaseInsensitive)) {
            return QStringLiteral("updateAvailable");
        }
        if (title.contains(QStringLiteral("Glucose Summary"), Qt::CaseInsensitive)
            || title.contains(QStringLiteral("Current Blood Glucose"), Qt::CaseInsensitive)
            || title.contains(QStringLiteral("Blood Sugar"), Qt::CaseInsensitive)) {
            return QStringLiteral("glucoseStatusSummary");
        }
        if (critical) {
            return QStringLiteral("lowGlucose");
        }
        return withSnoozeActions
            ? QStringLiteral("highGlucose")
            : QStringLiteral("glucoseStatusSummary");
    }

    bool isKdeNotificationEventPopupEnabled(const QString& eventId) const {
        if (eventId.isEmpty()) {
            return true;
        }
        QString cfgPath;
        const QString xdgConfig = qEnvironmentVariable("XDG_CONFIG_HOME").trimmed();
        if (!xdgConfig.isEmpty()) {
            const QString candidate = QDir(xdgConfig).filePath(
                QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.notifyrc")
            );
            if (QFile::exists(candidate)) {
                cfgPath = candidate;
            }
        }
        if (cfgPath.isEmpty()) {
            const QString hostCandidate = QDir::homePath()
                + QStringLiteral("/.config/io.github.russellmilliner.LibreLinkUpDesktop.notifyrc");
            if (QFile::exists(hostCandidate)) {
                cfgPath = hostCandidate;
            }
        }
        if (cfgPath.isEmpty()) {
            return true;
        }
        const KSharedConfig::Ptr cfg = KSharedConfig::openConfig(cfgPath, KConfig::SimpleConfig);
        if (!cfg) {
            return true;
        }
        cfg->reparseConfiguration();
        const KConfigGroup grp = cfg->group(QStringLiteral("Event/%1").arg(eventId));
        if (!grp.exists() || !grp.hasKey(QStringLiteral("Action"))) {
            return true;
        }
        const QString actionSpec = grp.readEntry(QStringLiteral("Action"), QStringLiteral("Popup")).trimmed();
        const QStringList tokens = actionSpec.split(QLatin1Char('|'), Qt::SkipEmptyParts);
        return tokens.contains(QStringLiteral("Popup"), Qt::CaseInsensitive);
    }

    // Sends desktop notifications over org.freedesktop.Notifications with KDE KNotifications
    // event IDs (`x-kde-eventId`, `x-kde-appname`, `x-kde-display-appname`) so KDE Plasma
    // System Settings → Notifications per-event toggles and history work identically across
    // native and Flatpak builds, falling back to KStatusNotifierItem/QSystemTrayIcon when D-Bus
    // is unavailable.
    void showNotification(
        const QString& title,
        const QString& body,
        bool critical,
        bool withSnoozeActions = false
    ) {
        ensureInitialized();
        const QString eventId = resolveNotificationEventId(title, critical, withSnoozeActions);
        if (!isKdeNotificationEventPopupEnabled(eventId)
            && eventId != QStringLiteral("urgentLowGlucose")) {
            return;
        }
        const QStringList actions = withSnoozeActions
            ? QStringList{
                QStringLiteral("default"), QStringLiteral("Open"),
                QStringLiteral("snooze_30"), QStringLiteral("Snooze 30 min"),
                QStringLiteral("snooze_60"), QStringLiteral("Snooze 1 hr"),
                QStringLiteral("snooze_120"), QStringLiteral("Snooze 2 hr"),
            }
            : QStringList{
                QStringLiteral("default"), QStringLiteral("Open"),
            };
        if (sendDesktopNotificationWithActions(
                title, body, critical, actions, withSnoozeActions, QString(), eventId)) {
            return;
        }
        if (m_sni) {
            m_sni->showMessage(
                title,
                body,
                critical ? QStringLiteral("dialog-error") : QStringLiteral("dialog-information"),
                15000
            );
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
    // Passes `medicationId` so any existing notification for that medication/slot is replaced
    // in-place instead of stacking duplicate banners.
    void showMedicationNotification(
        const QString& medicationId,
        const QString& title,
        const QString& body
    ) {
        ensureInitialized();
        if (!isKdeNotificationEventPopupEnabled(QStringLiteral("medicationDue"))) {
            return;
        }
        const QStringList medActions{
            QStringLiteral("default"), QStringLiteral("Open"),
            QStringLiteral("med_taken:%1").arg(medicationId), QStringLiteral("✓ Taken"),
            QStringLiteral("med_skip:%1").arg(medicationId), QStringLiteral("⏭ Skipped"),
        };
        if (sendDesktopNotificationWithActions(
                title, body, false, medActions, false, medicationId, QStringLiteral("medicationDue"))) {
            return;
        }
        if (m_sni) {
            m_sni->showMessage(
                title,
                body,
                QStringLiteral("dialog-information"),
                15000
            );
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
        for (auto it = m_activeMedicationNotificationIds.begin();
             it != m_activeMedicationNotificationIds.end();) {
            if (it.value() == id) {
                it = m_activeMedicationNotificationIds.erase(it);
            } else {
                ++it;
            }
        }
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
        for (auto it = m_activeMedicationNotificationIds.begin();
             it != m_activeMedicationNotificationIds.end();) {
            if (it.value() == id) {
                it = m_activeMedicationNotificationIds.erase(it);
            } else {
                ++it;
            }
        }
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

        if (m_controller && m_overlayOnlyMode != onlyOverlayShowing) {
            m_overlayOnlyMode = onlyOverlayShowing;
            ++m_overlayOnlyModeInvokeCount;
            QMetaObject::invokeMethod(
                m_controller,
                "updateOverlayOnlyMode",
                Qt::QueuedConnection,
                Q_ARG(bool, onlyOverlayShowing)
            );
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
    ) {
        m_controller = controller;
        m_overlayVisible = overlayVisible;
        m_overlayOnlyMode = overlayOnlyMode;
        ensureInitialized();
        if (!m_sni) {
            return;
        }

        const KStatusNotifierItem::ItemStatus desiredStatus = trayNeedsAttention
            ? KStatusNotifierItem::NeedsAttention
            : (trayIsRelevant ? KStatusNotifierItem::Active : KStatusNotifierItem::Passive);
        if (m_sni->status() != desiredStatus) {
            m_sni->setStatus(desiredStatus);
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
                m_currentTrayIcon = badgeIcon;
            } else {
                m_currentTrayIcon = baseAppIcon;
            }
            m_sni->setIconByPixmap(m_currentTrayIcon);
            m_sni->setAttentionIconByPixmap(m_currentTrayIcon);
            m_sni->setToolTipIconByPixmap(baseAppIcon);
        }

        if (m_lastTooltipText != tooltipText) {
            m_lastTooltipText = tooltipText;
            const QStringList lines = tooltipText.split(QLatin1Char('\n'));
            const QString title = lines.isEmpty()
                ? QStringLiteral("LibreLinkUp Rust Desktop")
                : lines.first();
            const QString subTitle = lines.size() > 1
                ? lines.mid(1).join(QLatin1Char('\n'))
                : QString();
            m_sni->setToolTipTitle(title);
            m_sni->setToolTipSubTitle(subTitle);
        }

        if (m_statusAction && m_lastStatusHeader != statusHeader) {
            m_lastStatusHeader = statusHeader;
            m_statusAction->setText(statusHeader);
        }
        if (m_rangeAction && m_lastRangeHeader != rangeHeader) {
            m_lastRangeHeader = rangeHeader;
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
        if (m_onlyRelevantOutOfRangeAction) {
            const bool old = m_onlyRelevantOutOfRangeAction->blockSignals(true);
            m_onlyRelevantOutOfRangeAction->setChecked(onlyRelevantOutOfRange);
            m_onlyRelevantOutOfRangeAction->blockSignals(old);
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

        m_sni = new KStatusNotifierItem(
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop"),
            QCoreApplication::instance()
        );
        m_sni->setCategory(KStatusNotifierItem::ApplicationStatus);
        m_sni->setTitle(QStringLiteral("LibreLinkUp Rust Desktop"));
        m_sni->setStandardActionsEnabled(false);
        m_sni->setStatus(KStatusNotifierItem::Active);
        m_currentTrayIcon = build_multisize_svg_icon(
            QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
        );
        m_sni->setIconByPixmap(m_currentTrayIcon);
        m_sni->setAttentionIconByPixmap(m_currentTrayIcon);
        m_sni->setToolTipIconByPixmap(m_currentTrayIcon);
        m_sni->setToolTipTitle(QStringLiteral("LibreLinkUp Rust Desktop"));
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

        m_onlyRelevantOutOfRangeAction = m_menu->addAction(
            QStringLiteral("Only Relevant in Tray When Out of Range")
        );
        m_onlyRelevantOutOfRangeAction->setCheckable(true);
        QObject::connect(m_onlyRelevantOutOfRangeAction, &QAction::triggered, [this](bool checked) {
            if (m_controller) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "updateTrayOnlyRelevantOutOfRange",
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
        m_sni->setContextMenu(m_menu);

        QObject::connect(
            m_sni,
            &KStatusNotifierItem::activateRequested,
            [](bool /*active*/, const QPoint& /*pos*/) {
                if (is_main_window_shown()) {
                    hide_main_window_to_tray();
                } else {
                    show_main_window();
                }
            }
        );

        ensureKdeNotificationConfigInstalled();
        registerKdeGlobalShortcuts();
    }

    static QByteArray canonicalKdeNotifyRcBytes() {
        return QByteArrayLiteral(
            "[Global]\n"
            "IconName=io.github.russellmilliner.LibreLinkUpDesktop\n"
            "Comment=Continuous Glucose Monitor & Medication Reminders\n"
            "Name=LibreLinkUp Rust Desktop\n"
            "DesktopEntry=io.github.russellmilliner.LibreLinkUpDesktop\n\n"
            "[Event/urgentLowGlucose]\n"
            "Name=Urgent Low Glucose Alert\n"
            "Comment=Critical alert when blood glucose drops at or below the urgent low threshold\n"
            "Action=Popup\n"
            "Urgency=Critical\n\n"
            "[Event/lowGlucose]\n"
            "Name=Low Glucose Alert\n"
            "Comment=Critical alert when blood glucose drops below the target low threshold\n"
            "Action=Popup\n"
            "Urgency=Critical\n\n"
            "[Event/highGlucose]\n"
            "Name=High Glucose Alert\n"
            "Comment=Alert when blood glucose rises above the target high threshold\n"
            "Action=Popup\n"
            "Urgency=High\n\n"
            "[Event/staleReading]\n"
            "Name=Stale Sensor Reading Alert\n"
            "Comment=Warning when no new CGM reading has been received within the configured stale threshold\n"
            "Action=Popup\n"
            "Urgency=Normal\n\n"
            "[Event/medicationDue]\n"
            "Name=Medication Due Reminder\n"
            "Comment=Scheduled reminder when one or more medications are due to be taken\n"
            "Action=Popup\n"
            "Urgency=Normal\n\n"
            "[Event/sensorExpiring]\n"
            "Name=CGM Sensor Expiring Soon\n"
            "Comment=Reminder when the active FreeStyle Libre sensor is approaching expiration\n"
            "Action=Popup\n"
            "Urgency=Normal\n\n"
            "[Event/glucoseStatusSummary]\n"
            "Name=On-Demand Glucose Reading Summary\n"
            "Comment=Popup notification showing the current blood glucose value, trend, and time in range when triggered by shortcut\n"
            "Action=Popup\n"
            "Urgency=Low\n\n"
            "[Event/updateAvailable]\n"
            "Name=Application Update Notification\n"
            "Comment=Notification when a newer version of LibreLinkUp Rust Desktop is available on GitHub Releases\n"
            "Action=Popup\n"
            "Urgency=Low\n\n"
            "[Event/alertTest]\n"
            "Name=Manual Alert Test\n"
            "Comment=Test notification triggered from the Alerts settings tab\n"
            "Action=Popup\n"
            "Urgency=Normal\n"
        );
    }

    void ensureKdeNotificationConfigInstalled() {
        if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
            return;
        }
        const QString dirPath = QDir::homePath() + QStringLiteral("/.local/share/knotifications6");
        QDir dir(dirPath);
        if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
            return;
        }
        const QString targetPath = dir.filePath(
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.notifyrc")
        );
        const QByteArray desired = canonicalKdeNotifyRcBytes();
        QFile existing(targetPath);
        if (existing.exists() && existing.open(QIODevice::ReadOnly)) {
            const QByteArray current = existing.readAll();
            existing.close();
            if (current == desired) {
                return;
            }
        }
        QSaveFile out(targetPath);
        if (out.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            out.write(desired);
            if (out.commit()) {
                QDBusConnection bus = QDBusConnection::sessionBus();
                if (bus.isConnected()) {
                    QDBusMessage reparseSignal = QDBusMessage::createSignal(
                        QStringLiteral("/Config"),
                        QStringLiteral("org.kde.knotification"),
                        QStringLiteral("reparseConfiguration")
                    );
                    reparseSignal << QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop");
                    bus.send(reparseSignal);
                }
            }
        }
    }

    void ensureNotificationWatcherConnected(QDBusConnection& bus) {
        if (m_notifWatcher || !QCoreApplication::instance()) {
            return;
        }
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

    void registerKdeGlobalShortcuts() {
        if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
            return;
        }
        QDBusConnection bus = QDBusConnection::sessionBus();
        if (!bus.isConnected()) {
            return;
        }
        ensureNotificationWatcherConnected(bus);

        const QString componentUnique = QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.desktop");
        const QString componentFriendly = QStringLiteral("LibreLinkUp Rust Desktop");
        const QString componentPath = QStringLiteral("/component/io_github_russellmilliner_LibreLinkUpDesktop_desktop");

        bus.connect(
            QStringLiteral("org.kde.kglobalaccel"),
            componentPath,
            QStringLiteral("org.kde.kglobalaccel.Component"),
            QStringLiteral("globalShortcutPressed"),
            m_notifWatcher,
            SLOT(onGlobalShortcutPressed(QString,QString,qlonglong))
        );

        QSet<QString> existingShortcuts;
        {
            QDBusMessage listMsg = QDBusMessage::createMethodCall(
                QStringLiteral("org.kde.kglobalaccel"),
                componentPath,
                QStringLiteral("org.kde.kglobalaccel.Component"),
                QStringLiteral("shortcutNames")
            );
            const QDBusMessage listReply = bus.call(listMsg, QDBus::Block, 500);
            if (listReply.type() == QDBusMessage::ReplyMessage && !listReply.arguments().isEmpty()) {
                const QStringList names = listReply.arguments().first().toStringList();
                for (const QString& n : names) {
                    existingShortcuts.insert(n);
                }
            }
        }

        const QList<QPair<QString, QString>> shortcuts = {
            {QStringLiteral("_launch"), QStringLiteral("Launch / Show LibreLinkUp Rust Desktop")},
            {QStringLiteral("ToggleMainWindow"), QStringLiteral("Toggle Main Window (Show / Minimize to Tray)")},
            {QStringLiteral("ShowMainWindow"), QStringLiteral("Show Main Window")},
            {QStringLiteral("MinimizeToTray"), QStringLiteral("Minimize Main Window to Tray")},
            {QStringLiteral("ToggleOverlay"), QStringLiteral("Toggle Floating Desktop Overlay Widget")},
            {QStringLiteral("ToggleOverlayAllDesktops"), QStringLiteral("Toggle Overlay on All Virtual Desktops")},
            {QStringLiteral("RefreshGlucoseNow"), QStringLiteral("Refresh Blood Glucose Reading Now")},
            {QStringLiteral("ShowPopupSummary"), QStringLiteral("Show Current Blood Glucose Reading Notification")},
            {QStringLiteral("OpenMedicationsTab"), QStringLiteral("Open Medications & Log Dose")},
            {QStringLiteral("MarkDueMedicationsTaken"), QStringLiteral("Mark Due Medications as Taken")},
            {QStringLiteral("SnoozeAlerts30m"), QStringLiteral("Snooze Glucose Alerts for 30 Minutes")},
            {QStringLiteral("SnoozeAlerts60m"), QStringLiteral("Snooze Glucose Alerts for 1 Hour")},
            {QStringLiteral("SnoozeAlerts120m"), QStringLiteral("Snooze Glucose Alerts for 2 Hours")},
            {QStringLiteral("CancelAlertSnooze"), QStringLiteral("Cancel Alert Snooze (Resume Alerts)")},
            {QStringLiteral("ToggleHighContrast"), QStringLiteral("Toggle WCAG AAA High Contrast Mode")},
            {QStringLiteral("OpenLogViewer"), QStringLiteral("Open Diagnostic Log Viewer")},
        };

        QDBusArgument emptyIntArray;
        emptyIntArray.beginArray(QMetaType::fromType<int>());
        emptyIntArray.endArray();
        const QVariant emptyKeysVar = QVariant::fromValue(emptyIntArray);

        for (const auto& entry : shortcuts) {
            const QStringList actionId{
                componentUnique,
                entry.first,
                componentFriendly,
                entry.second,
            };
            QDBusMessage regMsg = QDBusMessage::createMethodCall(
                QStringLiteral("org.kde.kglobalaccel"),
                QStringLiteral("/kglobalaccel"),
                QStringLiteral("org.kde.KGlobalAccel"),
                QStringLiteral("doRegister")
            );
            regMsg << actionId;
            bus.asyncCall(regMsg);

            if (!existingShortcuts.contains(entry.first)) {
                QDBusMessage setMsg = QDBusMessage::createMethodCall(
                    QStringLiteral("org.kde.kglobalaccel"),
                    QStringLiteral("/kglobalaccel"),
                    QStringLiteral("org.kde.KGlobalAccel"),
                    QStringLiteral("setForeignShortcut")
                );
                setMsg << actionId << emptyKeysVar;
                bus.asyncCall(setMsg);
            }
        }
    }

public:
    static void syncSecondaryControllers(Qt::ConnectionType connType) {
        if (QWindow* mainWin = find_main_window()) {
            if (QObject* settingsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
                QMetaObject::invokeMethod(settingsCtrl, "sync", connType);
            }
            if (QObject* medsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupMedicationsController"))) {
                QMetaObject::invokeMethod(medsCtrl, "sync", connType);
            }
        }
    }

    bool handleGlobalShortcutAction(const QString& actionId) {
        const QString cleanId = actionId.trimmed();
        if (cleanId.isEmpty()) {
            return false;
        }
        const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
        if (qEnvironmentVariable("QT_QPA_PLATFORM") != QStringLiteral("offscreen")) {
            if (cleanId == m_lastShortcutActionId && (nowMs - m_lastShortcutEpochMs) < 400) {
                return true;
            }
        }
        m_lastShortcutActionId = cleanId;
        m_lastShortcutEpochMs = nowMs;

        if (m_controller) {
            bool handled = false;
            if (QThread::currentThread() == m_controller->thread()) {
                QMetaObject::invokeMethod(
                    m_controller,
                    "handleGlobalShortcutAction",
                    Qt::DirectConnection,
                    Q_RETURN_ARG(bool, handled),
                    Q_ARG(QString, cleanId)
                );
                if (handled) {
                    syncSecondaryControllers(Qt::DirectConnection);
                }
                return handled;
            }
            QMetaObject::invokeMethod(
                m_controller,
                "handleGlobalShortcutAction",
                Qt::QueuedConnection,
                Q_ARG(QString, cleanId)
            );
            syncSecondaryControllers(Qt::QueuedConnection);
            return true;
        }

        if (cleanId == QStringLiteral("_launch") || cleanId == QStringLiteral("ShowMainWindow")) {
            show_main_window();
            return true;
        }
        if (cleanId == QStringLiteral("MinimizeToTray")) {
            hide_main_window_to_tray();
            return true;
        }
        if (cleanId == QStringLiteral("ToggleMainWindow")) {
            if (is_main_window_shown()) {
                hide_main_window_to_tray();
            } else {
                show_main_window();
            }
            return true;
        }
        if (cleanId == QStringLiteral("OpenMedicationsTab")) {
            show_main_window_tab(QStringLiteral("medications"));
            return true;
        }
        if (cleanId == QStringLiteral("OpenLogViewer")) {
            show_log_viewer_window();
            return true;
        }
        return false;
    }

private:

    bool m_initialized = false;
    bool m_overlayVisible = false;
    bool m_overlayOnlyMode = false;
    int m_overlayOnlyModeInvokeCount = 0;
    QPointer<QObject> m_controller;
    QPointer<KStatusNotifierItem> m_sni;
    QPointer<QSystemTrayIcon> m_tray;
    QIcon m_currentTrayIcon;
    QMenu* m_menu = nullptr;
    QAction* m_statusAction = nullptr;
    QAction* m_rangeAction = nullptr;
    QAction* m_showInIconAction = nullptr;
    QAction* m_showInTooltipAction = nullptr;
    QAction* m_onlyRelevantOutOfRangeAction = nullptr;
    QAction* m_overlayAction = nullptr;
    QAction* m_overlayAllDesktopsAction = nullptr;
    QAction* m_autostartAction = nullptr;
    QAction* m_themeSystemAction = nullptr;
    QAction* m_themeDarkAction = nullptr;
    QAction* m_themeLightAction = nullptr;
    QAction* m_showAction = nullptr;
    QAction* m_hideAction = nullptr;
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

    bool sendDesktopNotificationWithActions(
        const QString& title,
        const QString& body,
        bool critical,
        const QStringList& actions,
        bool replaceLastAlert,
        const QString& medicationKey = QString(),
        const QString& eventId = QString()
    ) {
        if (qEnvironmentVariable("QT_QPA_PLATFORM") == QStringLiteral("offscreen")) {
            return false;
        }
        QDBusConnection bus = QDBusConnection::sessionBus();
        if (!bus.isConnected()) {
            return false;
        }
        ensureNotificationWatcherConnected(bus);

        const QString resolvedEventId = eventId.isEmpty()
            ? resolveNotificationEventId(title, critical, replaceLastAlert, medicationKey)
            : eventId;
        const uchar urgencyLevel = critical
            ? 2u
            : ((resolvedEventId == QStringLiteral("glucoseStatusSummary")
                || resolvedEventId == QStringLiteral("updateAvailable"))
                ? 0u
                : 1u);

        QVariantMap hints;
        hints.insert(
            QStringLiteral("urgency"),
            QVariant::fromValue<uchar>(urgencyLevel)
        );
        hints.insert(
            QStringLiteral("desktop-entry"),
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop")
        );
        hints.insert(
            QStringLiteral("x-kde-appname"),
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop")
        );
        hints.insert(
            QStringLiteral("x-kde-display-appname"),
            QStringLiteral("LibreLinkUp Rust Desktop")
        );
        hints.insert(
            QStringLiteral("x-kde-eventId"),
            resolvedEventId
        );

        const uint replacesId = replaceLastAlert
            ? m_lastAlertNotificationId
            : (!medicationKey.isEmpty() ? m_activeMedicationNotificationIds.value(medicationKey, 0u) : 0u);

        QDBusMessage msg = QDBusMessage::createMethodCall(
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("/org/freedesktop/Notifications"),
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("Notify")
        );
        msg << QStringLiteral("LibreLinkUp Rust Desktop")
            << replacesId
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
            [this, title, body, critical, replaceLastAlert, medicationKey](QDBusPendingCallWatcher* w) {
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
                        if (!medicationKey.isEmpty()) {
                            m_activeMedicationNotificationIds.insert(medicationKey, notifId);
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

inline void NotificationActionWatcher::onGlobalShortcutPressed(
    const QString& componentUnique,
    const QString& shortcutUnique,
    qlonglong timestamp
) {
    Q_UNUSED(timestamp);
    if (componentUnique == QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.desktop")) {
        TrayManager::instance().handleGlobalShortcutAction(shortcutUnique);
    }
}

inline void NotificationActionWatcher::invokeGlobalAction(const QString& actionId) {
    TrayManager::instance().handleGlobalShortcutAction(actionId);
}

inline void NotificationActionWatcher::onKWinOverlayGeometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    handle_kwin_overlay_geometry(screenName, globalX, globalY, width, height);
}

inline void NotificationActionWatcher::onKWinMainWindowGeometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    handle_kwin_main_window_geometry(screenName, globalX, globalY, width, height);
}

inline QString NotificationActionWatcher::getDesiredOverlaySync() {
    return format_desired_overlay_sync();
}

inline QString NotificationActionWatcher::getDesiredMainSync() {
    return format_desired_main_sync();
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
