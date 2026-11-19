#pragma once

#include <QString>
#include <cstdint>

void librelinkup_play_alert_sound(const QString& custom_sound_path);
bool librelinkup_request_background_autostart(bool enable, bool start_minimized);
bool librelinkup_try_dispatch_cli_action(const QString& action_id);
void librelinkup_quit_app();
std::int32_t librelinkup_run_app(bool start_minimized, const QString& initial_action);
