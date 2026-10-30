use librelinkup_kde::bridge::qobject_bridge::qobject::librelinkup_run_app;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!(
            "LibreLinkUp Desktop (KDE Plasma 6) v{}",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "LibreLinkUp Desktop — KDE Plasma 6 Continuous Glucose Monitor\n\n\
             Usage: librelinkup-kde [OPTIONS]\n\n\
             Options:\n  \
               -m, --minimized, --start-minimized, --autostart   Start hidden in the system tray\n  \
               -u, --check-update                                Check GitHub Releases for updates and exit\n  \
               -V, --version                                     Print version information\n  \
               -h, --help                                        Print this help message"
        );
        return;
    }

    if args.iter().any(|a| a == "--check-update" || a == "-u") {
        match librelinkup_kde::updater::check_for_updates_blocking(env!("CARGO_PKG_VERSION")) {
            Ok(res) => {
                println!("{}", res.message);
                println!("Release page: {}", res.release_url);
                if res.update_available {
                    println!("Download URL: {}", res.download_url);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let start_minimized = args.iter().any(|a| {
        a == "--minimized"
            || a == "-m"
            || a == "--start-minimized"
            || a == "--autostart"
            || a == "--hidden"
    }) || std::env::var("START_MINIMIZED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let exit_code = librelinkup_run_app(start_minimized);
    std::process::exit(exit_code);
}
