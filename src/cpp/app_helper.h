#pragma once

#include <KColorScheme>
#include <KColorSchemeManager>
#include <KIconLoader>
#include <KSharedConfig>
#include <QAbstractItemModel>
#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QColor>
#include <QCoreApplication>
#include <QDateTime>
#include <QDir>
#include <QEvent>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QGuiApplication>
#include <QIcon>
#include <QImage>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QLoggingCategory>
#include <QMenu>
#include <QMetaObject>
#include <QModelIndex>
#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <QPointer>
#include <QProcess>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QScreen>
#include <QStandardPaths>
#include <QString>
#include <QStringList>
#include <QSvgRenderer>
#include <QSystemTrayIcon>
#include <QThread>
#include <QTimer>
#include <QUrl>
#include <QVariant>
#include <QWindow>
#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <unistd.h>

namespace librelinkup {

inline QPointer<QWindow>& main_window_ref() {
    static QPointer<QWindow> win;
    return win;
}

inline QWindow* find_main_window();
inline QWindow* find_overlay_window();
inline void update_tray_window_actions();

inline QIcon build_multisize_svg_icon(const QString& qrcPath) {
    QSvgRenderer renderer(qrcPath);
    QIcon icon;
    const int sizes[] = {16, 22, 24, 32, 48, 64, 128};
    for (int sz : sizes) {
        QPixmap pm(sz, sz);
        pm.fill(Qt::transparent);
        if (renderer.isValid()) {
            QPainter painter(&pm);
            painter.setRenderHint(QPainter::Antialiasing, true);
            painter.setRenderHint(QPainter::SmoothPixmapTransform, true);
            renderer.render(&painter, QRectF(0, 0, sz, sz));
        }
        icon.addPixmap(pm);
    }
    return icon;
}

inline QIcon build_glucose_badge_icon(
    const QString& badgeText,
    const QString& trendSymbol,
    const QString& colorHex
) {
    QColor bg(colorHex);
    if (!bg.isValid()) {
        bg = QColor(34, 197, 94);
    }
    QIcon icon;
    const int sizes[] = {16, 22, 24, 32, 48, 64, 128};
    for (int sz : sizes) {
        QPixmap pm(sz, sz);
        pm.fill(Qt::transparent);
        QPainter p(&pm);
        p.setRenderHint(QPainter::Antialiasing, true);
        p.setRenderHint(QPainter::TextAntialiasing, true);

        const qreal margin = std::max(0.5, sz * 0.04);
        const QRectF rect(margin, margin, sz - margin * 2.0, sz - margin * 2.0);
        const qreal radius = sz * 0.24;

        p.setPen(QPen(QColor(255, 255, 255, 90), std::max(1.0, sz * 0.04)));
        p.setBrush(bg);
        p.drawRoundedRect(rect, radius, radius);

        p.setPen(Qt::white);
        QFont f = p.font();
        f.setBold(true);

        const int len = badgeText.length();
        double scale = 0.50;
        if (len >= 4) {
            scale = 0.34;
        } else if (len == 3) {
            scale = 0.41;
        }

        if (sz >= 32 && !trendSymbol.isEmpty()) {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale * 0.88)));
            p.setFont(f);
            QRectF topRect(rect.left(), rect.top() + sz * 0.04, rect.width(), rect.height() * 0.62);
            p.drawText(topRect, Qt::AlignCenter, badgeText);

            QFont arrowFont = f;
            arrowFont.setPixelSize(std::max(7, static_cast<int>(sz * 0.26)));
            p.setFont(arrowFont);
            QRectF botRect(rect.left(), rect.top() + rect.height() * 0.54, rect.width(), rect.height() * 0.42);
            p.drawText(botRect, Qt::AlignCenter, trendSymbol);
        } else {
            f.setPixelSize(std::max(8, static_cast<int>(sz * scale)));
            p.setFont(f);
            p.drawText(rect, Qt::AlignCenter, badgeText);
        }
        p.end();
        icon.addPixmap(pm);
    }
    return icon;
}

inline void ensure_window_hooks(QWindow* win) {
    if (!win || win->property("_llu_minimize_hooked").toBool()) {
        return;
    }
    win->setProperty("_llu_minimize_hooked", true);

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

inline QWindow* find_overlay_window() {
    const auto windows = QGuiApplication::topLevelWindows();
    for (QWindow* w : windows) {
        if (w && w->objectName() == QStringLiteral("librelinkupOverlayWindow")) {
            if (w->transientParent() != nullptr) {
                w->setTransientParent(nullptr);
            }
            return w;
        }
    }
    return nullptr;
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
        win->showNormal();
        win->show();
        win->raise();
        win->requestActivate();
    }
    update_tray_window_actions();
}

inline void hide_main_window_to_tray() {
    if (QWindow* win = find_main_window()) {
        win->hide();
        if (win->windowStates() & Qt::WindowMinimized) {
            win->setWindowStates(win->windowStates() & ~Qt::WindowMinimized);
        }
    }
    update_tray_window_actions();
}

inline void show_log_viewer_window() {
    if (QWindow* logWin = find_log_viewer_window()) {
        logWin->showNormal();
        logWin->show();
        logWin->raise();
        logWin->requestActivate();
        return;
    }
    if (QWindow* mainWin = find_main_window()) {
        QMetaObject::invokeMethod(mainWin, "openLogViewer", Qt::QueuedConnection);
    }
}

class TrayManager {
public:
    static TrayManager& instance() {
        static TrayManager inst;
        return inst;
    }

    void updateWindowActionsVisibility() {
        const bool shown = is_main_window_shown();
        if (m_showAction) {
            m_showAction->setVisible(!shown);
        }
        if (m_hideAction) {
            m_hideAction->setVisible(shown);
        }
    }

    void sync(
        QObject* controller,
        bool useLightOnDarkIcon,
        bool showGlucoseInIcon,
        bool showGlucoseInTooltip,
        bool autostartEnabled,
        bool overlayVisible,
        const QString& appThemeMode,
        const QString& badgeText,
        const QString& trendSymbol,
        const QString& badgeColorHex,
        const QString& statusHeader,
        const QString& rangeHeader,
        const QString& tooltipText
    ) {
        m_controller = controller;
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

        if (!m_tray->isVisible()) {
            m_tray->show();
        }
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

        m_showAction = m_menu->addAction(QStringLiteral("Show LibreLinkUp Desktop"));
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
            QStringLiteral("Quit LibreLinkUp Desktop")
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
    QPointer<QObject> m_controller;
    QPointer<QSystemTrayIcon> m_tray;
    QMenu* m_menu = nullptr;
    QAction* m_statusAction = nullptr;
    QAction* m_rangeAction = nullptr;
    QAction* m_showInIconAction = nullptr;
    QAction* m_showInTooltipAction = nullptr;
    QAction* m_overlayAction = nullptr;
    QAction* m_autostartAction = nullptr;
    QAction* m_themeSystemAction = nullptr;
    QAction* m_themeDarkAction = nullptr;
    QAction* m_themeLightAction = nullptr;
    QAction* m_showAction = nullptr;
    QAction* m_hideAction = nullptr;
    QString m_lastIconKey;
};

inline void update_tray_window_actions() {
    TrayManager::instance().updateWindowActionsVisibility();
}

} // namespace librelinkup

template <typename ControllerT>
inline void librelinkup_sync_tray(
    ControllerT& controller,
    const QString& app_theme_mode,
    const QString& badge_text,
    const QString& trend_symbol,
    const QString& badge_color_hex,
    const QString& status_header,
    const QString& range_header,
    const QString& tooltip_text,
    std::uint8_t flags
) {
    const bool useLightOnDarkIcon = (flags & 0x01u) != 0u;
    const bool showGlucoseInIcon = (flags & 0x02u) != 0u;
    const bool showGlucoseInTooltip = (flags & 0x04u) != 0u;
    const bool autostartEnabled = (flags & 0x08u) != 0u;
    const bool overlayVisible = (flags & 0x10u) != 0u;

    librelinkup::TrayManager::instance().sync(
        &controller,
        useLightOnDarkIcon,
        showGlucoseInIcon,
        showGlucoseInTooltip,
        autostartEnabled,
        overlayVisible,
        app_theme_mode,
        badge_text,
        trend_symbol,
        badge_color_hex,
        status_header,
        range_header,
        tooltip_text
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

inline void librelinkup_show_main_window() {
    librelinkup::show_main_window();
}

inline void librelinkup_hide_main_window() {
    librelinkup::hide_main_window_to_tray();
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

inline void librelinkup_play_alert_sound(const QString& custom_sound_path) {
    const QString clean = custom_sound_path.trimmed();
    if (!clean.isEmpty() && QFile::exists(clean)) {
        if (!QProcess::startDetached(QStringLiteral("pw-play"), {clean})) {
            QProcess::startDetached(QStringLiteral("paplay"), {clean});
        }
        return;
    }
    QApplication::beep();
}

inline void librelinkup_quit_app() {
    QCoreApplication::quit();
}

inline int librelinkup_run_app(bool start_minimized) {
    QLoggingCategory::setFilterRules(QStringLiteral("union.*=false\nqt.core.qobject.connect=false"));
    if (!std::getenv("QT_QUICK_CONTROLS_STYLE")) {
        qputenv("QT_QUICK_CONTROLS_STYLE", "org.kde.breeze");
    }
    if (start_minimized) {
        qputenv("LIBRELINKUP_START_MINIMIZED", "1");
    }

    static int argc = 1;
    static char arg0[] = "librelinkup-kde";
    static char* argv[] = {arg0, nullptr};

    QApplication app(argc, argv);
    QGuiApplication::setApplicationName(QStringLiteral("LibreLinkUp Desktop"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("LibreLinkUp Desktop"));
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
            win->show();
            win->raise();
            win->requestActivate();
            librelinkup::update_tray_window_actions();
        }
    }

    return app.exec();
}
