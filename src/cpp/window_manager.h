#pragma once

#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QString>
#include <QWindow>
#include <cstdint>

class QScreen;

namespace librelinkup {

QPointer<QWindow>& main_window_ref();

QWindow* find_main_window();
QWindow* find_overlay_window();
void update_tray_window_actions();
void record_main_window_placement(QWindow* win, bool persistToSettings = true);
bool is_wayland_platform();
void ensure_kwin_dbus_bridge();
void run_kwin_oneshot_script(const QString& jsBody);
void sync_kwin_overlay_window(int globalX, int globalY, int w, int h, bool onAllDesktops);
void sync_kwin_main_window(int globalX, int globalY, int w, int h);
void query_kwin_overlay_geometry();
void query_kwin_main_window_geometry();

struct MainWindowPlacementState {
    QString screenName;
    int relX = -1;
    int relY = -1;
    int width = 980;
    int height = 680;
    bool hasPlacement = false;
    bool applying = false;

    static MainWindowPlacementState& instance();
    bool hasSavedPosition() const;
    void applyToWindow(QWindow* win, bool preserveCurrentSize = false);
};

void ensure_window_hooks(QWindow* win);
void clamp_window_to_screen(QWindow* win);
void restore_main_window_placement(
    const QString& screen_name,
    int x,
    int y,
    int width,
    int height
);

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

    static OverlayPlacementState& instance();
    void invalidateKWinSync() const;
    bool computeGlobalGeometry(
        QWindow* ovWin,
        QScreen*& outScreen,
        int& outX,
        int& outY,
        int& outW,
        int& outH
    ) const;
    void applyToWindow(QWindow* ovWin, bool forceKWinSync = false) const;
};

QString kwin_script_dir();
bool is_safe_kwin_dbus_identifier(const QString& ident);
bool write_private_kwin_script(
    const QString& path,
    const QByteArray& bytes,
    bool exclusive
);
QString kwin_dbus_interface_name();
QString format_desired_overlay_sync();
QString format_desired_main_sync();
void stop_and_unload_resident_script();
QString build_kwin_resident_script_js(
    const QString& dbusAddr,
    int initOvX,
    int initOvY,
    int initOvW,
    int initOvH,
    const QString& dbusIface
);
void install_kwin_resident_script();
void handle_kwin_overlay_geometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
);
void handle_kwin_main_window_geometry(
    const QString& screenName,
    int globalX,
    int globalY,
    int width,
    int height
);
void ensure_overlay_hooks(QWindow* ovWin);
void restore_overlay_placement(
    const QString& screen_name,
    int x,
    int y,
    int width,
    int height
);
void apply_overlay_all_desktops(bool onAllDesktops);
void begin_overlay_drag();
void update_overlay_drag();
void end_overlay_drag(QObject* controller);
QWindow* find_log_viewer_window();
QWindow* find_about_window();
bool is_main_window_shown();
void show_main_window();
void hide_main_window_to_tray();
void show_log_viewer_window();
void show_about_window();
void show_main_window_tab(const QString& tabName);
void install_wheel_scroll_filter();

} // namespace librelinkup

bool librelinkup_is_main_window_shown();
void librelinkup_show_main_window();
void librelinkup_show_main_window_tab(const QString& tab_name);
void librelinkup_hide_main_window();
void librelinkup_restore_overlay_placement(
    const QString& screen_name,
    std::int32_t x,
    std::int32_t y,
    std::int32_t width,
    std::int32_t height
);
void librelinkup_restore_main_window_placement(
    const QString& screen_name,
    std::int32_t x,
    std::int32_t y,
    std::int32_t width,
    std::int32_t height
);
void librelinkup_apply_overlay_all_desktops(bool on_all_desktops);
void librelinkup_begin_overlay_drag();
void librelinkup_update_overlay_drag();

template <typename ControllerT>
inline void librelinkup_end_overlay_drag(ControllerT& controller) {
    librelinkup::end_overlay_drag(&controller);
}

void librelinkup_trigger_window_alert(bool bring_to_front, bool flash_window);
