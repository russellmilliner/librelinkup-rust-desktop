#include "app_helper.h"

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
#include <QList>
#include <QMenu>
#include <QMetaObject>
#include <QMetaType>
#include <QModelIndex>
#include <QPair>
#include <QPalette>
#include <QSaveFile>
#include <QStandardPaths>
#include <QSystemTrayIcon>
#include <QThread>
#include <QUrl>
#include <QVariant>
#include <QWindow>

namespace librelinkup {

TrayManager& TrayManager::instance() {
    static TrayManager inst;
    return inst;
}

bool TrayManager::isTrayIconVisible() const {
    return m_sni != nullptr || (m_tray && m_tray->isVisible());
}

int TrayManager::statusNotifierStatus() const {
    return m_sni ? static_cast<int>(m_sni->status()) : 0;
}

int TrayManager::overlayOnlyModeInvokeCount() const {
    return m_overlayOnlyModeInvokeCount;
}

QObject* TrayManager::settingsController() const {
    if (QWindow* mainWin = find_main_window()) {
        if (QObject* settingsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
            return settingsCtrl;
        }
    }
    return m_controller.data();
}

void TrayManager::popupMenuAtCursor() {
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

QString TrayManager::resolveNotificationEventId(
    const QString& title,
    bool critical,
    bool withSnoozeActions,
    const QString& medicationKey
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

bool TrayManager::isKdeNotificationEventPopupEnabled(
    const QString& eventId,
    const QString& configDirOverride
) const {
    if (eventId.isEmpty()) {
        return true;
    }
    QString cfgPath;
    const QString explicitDir = configDirOverride.trimmed();
    if (!explicitDir.isEmpty()) {
        const QString candidate = QDir(explicitDir).filePath(
            QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.notifyrc")
        );
        if (QFile::exists(candidate)) {
            cfgPath = candidate;
        }
    }
    if (cfgPath.isEmpty()) {
        const QString xdgConfig = qEnvironmentVariable("XDG_CONFIG_HOME").trimmed();
        if (!xdgConfig.isEmpty()) {
            const QString candidate = QDir(xdgConfig).filePath(
                QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.notifyrc")
            );
            if (QFile::exists(candidate)) {
                cfgPath = candidate;
            }
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

void TrayManager::showNotification(
    const QString& title,
    const QString& body,
    bool critical,
    bool withSnoozeActions
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

void TrayManager::showMedicationNotification(
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

void TrayManager::handleNotificationAction(uint id, const QString& actionKey) {
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

void TrayManager::handleNotificationClosed(uint id) {
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

void TrayManager::updateWindowActionsVisibility() {
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

    if (QObject* settingsCtrl = settingsController(); settingsCtrl && m_overlayOnlyMode != onlyOverlayShowing) {
        m_overlayOnlyMode = onlyOverlayShowing;
        ++m_overlayOnlyModeInvokeCount;
        QMetaObject::invokeMethod(
            settingsCtrl,
            "updateOverlayOnlyMode",
            Qt::QueuedConnection,
            Q_ARG(bool, onlyOverlayShowing)
        );
    }

    if (m_tray && !m_tray->isVisible()) {
        m_tray->show();
    }
}

void TrayManager::sync(
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
        QIcon& cachedBase = useLightOnDarkIcon ? m_cachedDarkBaseIcon : m_cachedLightBaseIcon;
        if (cachedBase.isNull()) {
            cachedBase = build_multisize_svg_icon(
                useLightOnDarkIcon
                    ? QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-dark.svg")
                    : QStringLiteral(":/qt/qml/io/github/librelinkup/src/qml/icons/tray-icon-light.svg")
            );
        }
        const QIcon& baseAppIcon = cachedBase;
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
        m_sni->setToolTipSubTitle(subTitle.toHtmlEscaped());
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

void TrayManager::ensureInitialized() {
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
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateShowGlucoseInTrayIcon",
                Qt::QueuedConnection,
                Q_ARG(bool, checked)
            );
        }
    });

    m_showInTooltipAction = m_menu->addAction(QStringLiteral("Show Glucose Info on Tray Hover"));
    m_showInTooltipAction->setCheckable(true);
    QObject::connect(m_showInTooltipAction, &QAction::triggered, [this](bool checked) {
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
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
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateTrayOnlyRelevantOutOfRange",
                Qt::QueuedConnection,
                Q_ARG(bool, checked)
            );
        }
    });

    m_overlayAction = m_menu->addAction(QStringLiteral("Show Floating Desktop Overlay"));
    m_overlayAction->setCheckable(true);
    QObject::connect(m_overlayAction, &QAction::triggered, [this](bool checked) {
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateOverlayVisible",
                Qt::QueuedConnection,
                Q_ARG(bool, checked)
            );
        }
    });

    m_overlayAllDesktopsAction = m_menu->addAction(QStringLiteral("Show Overlay on All Desktops"));
    m_overlayAllDesktopsAction->setCheckable(true);
    QObject::connect(m_overlayAllDesktopsAction, &QAction::triggered, [this](bool checked) {
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
                "updateOverlayOnAllDesktops",
                Qt::QueuedConnection,
                Q_ARG(bool, checked)
            );
        }
    });

    m_autostartAction = m_menu->addAction(QStringLiteral("Start Minimized on Login"));
    m_autostartAction->setCheckable(true);
    QObject::connect(m_autostartAction, &QAction::triggered, [this](bool checked) {
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
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
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
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
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
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
        if (QObject* settingsCtrl = settingsController()) {
            QMetaObject::invokeMethod(
                settingsCtrl,
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
    QObject::connect(quitAction, &QAction::triggered, [this]() {
        if (m_controller) {
            QMetaObject::invokeMethod(m_controller, "quitApplication", Qt::QueuedConnection);
        } else {
            QCoreApplication::quit();
        }
    });

    updateWindowActionsVisibility();
    m_sni->setContextMenu(m_menu.data());

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

QByteArray TrayManager::canonicalKdeNotifyRcBytes() {
    QFile f(QStringLiteral(
        ":/qt/qml/io/github/librelinkup/data/io.github.russellmilliner.LibreLinkUpDesktop.notifyrc"
    ));
    if (f.open(QIODevice::ReadOnly)) {
        return f.readAll();
    }
    return QByteArray();
}

void TrayManager::ensureKdeNotificationConfigInstalled() {
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
    if (desired.isEmpty()) {
        return;
    }
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

void TrayManager::ensureNotificationWatcherConnected(QDBusConnection& bus) {
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

void TrayManager::registerKdeGlobalShortcuts() {
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

void TrayManager::syncSecondaryControllers(Qt::ConnectionType connType) {
    if (QWindow* mainWin = find_main_window()) {
        if (QObject* settingsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupSettingsController"))) {
            QMetaObject::invokeMethod(settingsCtrl, "sync", connType);
        }
        if (QObject* medsCtrl = mainWin->findChild<QObject*>(QStringLiteral("librelinkupMedicationsController"))) {
            QMetaObject::invokeMethod(medsCtrl, "sync", connType);
        }
    }
}

bool TrayManager::handleGlobalShortcutAction(const QString& actionId) {
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

bool TrayManager::sendDesktopNotificationWithActions(
    const QString& title,
    const QString& body,
    bool critical,
    const QStringList& actions,
    bool replaceLastAlert,
    const QString& medicationKey,
    const QString& eventId
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
        << body.toHtmlEscaped()
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
                if (m_sni) {
                    m_sni->showMessage(
                        title,
                        body,
                        critical ? QStringLiteral("dialog-error") : QStringLiteral("dialog-information"),
                        15000
                    );
                } else if (m_tray && QSystemTrayIcon::supportsMessages()) {
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

void NotificationActionWatcher::onActionInvoked(uint id, const QString& actionKey) {
    TrayManager::instance().handleNotificationAction(id, actionKey);
}

void NotificationActionWatcher::onNotificationClosed(uint id, uint reason) {
    Q_UNUSED(reason);
    TrayManager::instance().handleNotificationClosed(id);
}

void NotificationActionWatcher::onGlobalShortcutPressed(
    const QString& componentUnique,
    const QString& shortcutUnique,
    qlonglong timestamp
) {
    Q_UNUSED(timestamp);
    if (componentUnique == QStringLiteral("io.github.russellmilliner.LibreLinkUpDesktop.desktop")) {
        TrayManager::instance().handleGlobalShortcutAction(shortcutUnique);
    }
}

void NotificationActionWatcher::invokeGlobalAction(const QString& actionId) {
    TrayManager::instance().handleGlobalShortcutAction(actionId);
}

void NotificationActionWatcher::onKWinOverlayGeometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    handle_kwin_overlay_geometry(screenName, globalX, globalY, width, height);
}

void NotificationActionWatcher::onKWinMainWindowGeometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
) {
    handle_kwin_main_window_geometry(screenName, globalX, globalY, width, height);
}

QString NotificationActionWatcher::getDesiredOverlaySync() {
    return format_desired_overlay_sync();
}

QString NotificationActionWatcher::getDesiredMainSync() {
    return format_desired_main_sync();
}

void update_tray_window_actions() {
    TrayManager::instance().updateWindowActionsVisibility();
}

} // namespace librelinkup

void librelinkup_apply_app_theme(const QString& themeMode) {
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

void librelinkup_popup_tray_menu() {
    librelinkup::TrayManager::instance().popupMenuAtCursor();
}

void librelinkup_show_notification(
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

void librelinkup_show_medication_notification(
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
