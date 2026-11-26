#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> 1. Checking Rust formatting (root workspace & windows/)..."
cargo fmt --all -- --check
cargo fmt --manifest-path windows/Cargo.toml -- --check

echo "==> 2. Checking Rust clippy (default, --no-default-features, and windows/)..."
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
if [[ "$(uname -s)" == *MINGW* || "$(uname -s)" == *MSYS* ]] || pkg-config --exists alsa 2>/dev/null; then
    cargo clippy --manifest-path windows/Cargo.toml --all-targets -- -D warnings
else
    echo "  SKIP windows/ clippy (no ALSA headers; covered natively on Windows CI)"
fi

echo "==> 3. Running Rust unit, headless QML, and --ui-test suites..."
QT_QPA_PLATFORM=offscreen cargo test --workspace -- --test-threads=1
QT_QPA_PLATFORM=offscreen cargo test --workspace --no-default-features -- --test-threads=1
QT_QPA_PLATFORM=offscreen cargo run --locked -- --ui-test
if [[ "$(uname -s)" == *MINGW* || "$(uname -s)" == *MSYS* ]] || pkg-config --exists alsa 2>/dev/null; then
    cargo test --manifest-path windows/Cargo.toml
else
    echo "  SKIP windows/ tests (no ALSA headers; covered natively on Windows CI)"
fi

echo "==> 4. Linting QML files with Qt 6 qmllint..."
QMLLINT_BIN=""
for cand in /usr/lib/qt6/bin/qmllint "$(command -v qmllint6 2>/dev/null || true)" "$(command -v qmllint 2>/dev/null || true)"; do
    [[ -z "$cand" || ! -x "$cand" ]] && continue
    if "$cand" --version 2>/dev/null | grep -qE '^qmllint 6\.'; then
        QMLLINT_BIN="$cand"
        break
    fi
done
if [[ -n "$QMLLINT_BIN" ]]; then
    QML_IMPORT_DIR=""
    for qm in "$(command -v qmake6 2>/dev/null || true)" /usr/lib/qt6/bin/qmake "$(command -v qmake 2>/dev/null || true)"; do
        [[ -z "$qm" || ! -x "$qm" ]] && continue
        QML_IMPORT_DIR="$("$qm" -query QT_INSTALL_QML 2>/dev/null || true)"
        if [[ -n "$QML_IMPORT_DIR" && -d "$QML_IMPORT_DIR" ]]; then
            break
        fi
        QML_IMPORT_DIR=""
    done
    if [[ -z "$QML_IMPORT_DIR" ]]; then
        for d in /usr/lib/qt6/qml /usr/lib/x86_64-linux-gnu/qt6/qml; do
            if [[ -d "$d" ]]; then
                QML_IMPORT_DIR="$d"
                break
            fi
        done
    fi
    if [[ -n "$QML_IMPORT_DIR" ]]; then
        "$QMLLINT_BIN" -I "$QML_IMPORT_DIR" -I src/qml src/qml/*.qml
    else
        "$QMLLINT_BIN" -I src/qml src/qml/*.qml
    fi
else
    echo "  SKIP: Qt 6 qmllint not installed"
fi

echo "==> 5. Checking C++17 / Qt6 / KF6 sources with -Wall -Wextra -Werror..."
if command -v g++ >/dev/null 2>&1 && pkg-config --exists Qt6Core Qt6Gui Qt6Widgets Qt6Quick Qt6Qml Qt6Svg Qt6Network Qt6DBus 2>/dev/null; then
    QT_CFLAGS="$(pkg-config --cflags Qt6Core Qt6Gui Qt6Widgets Qt6Quick Qt6Qml Qt6Svg Qt6Network Qt6DBus)"
    KF6_CFLAGS="$(pkg-config --cflags KF6ColorScheme KF6ConfigCore KF6IconThemes KF6StatusNotifierItem 2>/dev/null || true)"
    for base in "${KF6_INCLUDE_DIR:-}" /usr/include/KF6 /usr/local/include/KF6 /usr/include/x86_64-linux-gnu/KF6; do
        [[ -z "$base" || ! -d "$base" ]] && continue
        for mod in KColorScheme KConfig KConfigCore KIconThemes KStatusNotifierItem; do
            if [[ -d "$base/$mod" && "$KF6_CFLAGS" != *"-I$base/$mod"* ]]; then
                KF6_CFLAGS="$KF6_CFLAGS -I$base/$mod"
            fi
        done
    done
    if [[ -n "$KF6_CFLAGS" ]]; then
        # shellcheck disable=SC2086
        echo '#include "app_helper.h"' | g++ -std=c++17 -fsyntax-only -Wall -Wextra -Werror -Isrc/cpp $QT_CFLAGS $KF6_CFLAGS -x c++ -
        CPP_FILES=(src/cpp/*.cpp)
        for cpp in "${CPP_FILES[@]}"; do
            # shellcheck disable=SC2086
            g++ -std=c++17 -fsyntax-only -Wall -Wextra -Werror -Isrc/cpp $QT_CFLAGS $KF6_CFLAGS "$cpp"
        done
        echo "  OK: app_helper.h and ${#CPP_FILES[@]} C++ translation unit(s)"
    else
        echo "  SKIP C++ check: KF6 headers not found"
    fi
else
    echo "  SKIP: g++ or Qt6 pkg-config development headers not installed"
fi

echo "==> 6. Checking shell scripts..."
SHELL_FILES=(scripts/*.sh windows/scripts/*.sh .githooks/pre-commit)
for shf in "${SHELL_FILES[@]}"; do
    bash -n "$shf"
done
if command -v shellcheck >/dev/null 2>&1; then
    shellcheck -S warning "${SHELL_FILES[@]}"
else
    echo "  NOTE: shellcheck not installed (verified bash -n on ${#SHELL_FILES[@]} script(s))"
fi

echo "==> 7. Checking KWin QML/JS helper script..."
if command -v node >/dev/null 2>&1; then
    node --check src/cpp/resources/kwin-resident.js
    echo "  OK: src/cpp/resources/kwin-resident.js"
else
    echo "  SKIP: node not installed"
fi

echo "==> 8. Validating Desktop Entry & AppStream XML metadata..."
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate data/io.github.russellmilliner.LibreLinkUpDesktop.desktop
    echo "  OK: desktop-file-validate passed"
else
    echo "  SKIP: desktop-file-validate not installed"
fi
if command -v appstreamcli >/dev/null 2>&1; then
    appstreamcli validate --no-net data/io.github.russellmilliner.LibreLinkUpDesktop.metainfo.xml
    echo "  OK: appstreamcli validate passed"
else
    echo "  SKIP: appstreamcli not installed"
fi
if command -v xmllint >/dev/null 2>&1; then
    xmllint --noout data/io.github.russellmilliner.LibreLinkUpDesktop.metainfo.xml
    echo "  OK: xmllint passed"
else
    echo "  SKIP: xmllint not installed"
fi

echo "==> 9. Verifying paths referenced in docs/ARCHITECTURE.md exist..."
if [[ ! -f docs/ARCHITECTURE.md ]]; then
    echo "ERROR: docs/ARCHITECTURE.md is missing!" >&2
    exit 1
fi
missing=0
while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    [[ "$path" == *"*"* ]] && continue
    if [[ ! -e "$path" ]]; then
        echo "ERROR: Path mentioned in docs/ARCHITECTURE.md does not exist: $path" >&2
        missing=1
    else
        echo "  OK: $path"
    fi
done < <(grep -hoE '(crates/[A-Za-z0-9_./-]+|src/[A-Za-z0-9_./-]+|windows/[A-Za-z0-9_./-]+)' docs/ARCHITECTURE.md | sed 's/[.,:;)`]*$//' | sort -u)

if [[ "$missing" -ne 0 ]]; then
    exit 1
fi

echo "==> All checks passed!"
