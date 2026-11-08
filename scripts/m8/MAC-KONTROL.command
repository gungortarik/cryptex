#!/bin/bash
# Portable macOS readiness receipt. No installation, network or Cryptex access.
set -eu
umask 077

if [ "$(uname -s)" != "Darwin" ]; then
    echo "Bu dosya gercek Mac uzerinde calistirilmalidir."
    exit 2
fi

work="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/cryptex-mac-kontrol.XXXXXX")"
report="$work/platform.txt"
result="$work/MAC-SONUC.zip"

{
    echo 'schema=1'
    echo 'scope=macos_readiness_only'
    echo 'probe_id=m8-macos-readiness-20261108-v1'
    echo "collected_utc=$(/bin/date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "macos_version=$(/usr/bin/sw_vers -productVersion)"
    echo "macos_build=$(/usr/bin/sw_vers -buildVersion)"
    echo "architecture=$(/usr/bin/uname -m)"
    translated="$(/usr/sbin/sysctl -in sysctl.proc_translated 2>/dev/null || true)"
    case "$translated" in
        0|1) echo "translated=$translated" ;;
        *) echo 'translated=unknown' ;;
    esac
    filesystem="$(/usr/sbin/diskutil info -plist "$work" 2>/dev/null |
        /usr/bin/plutil -extract FilesystemType raw -o - - 2>/dev/null || printf 'unknown')"
    case "$filesystem" in
        apfs|hfs|exfat|msdos|ufs) echo "scratch_filesystem=$filesystem" ;;
        *) echo 'scratch_filesystem=unknown' ;;
    esac
    if /usr/bin/xcode-select -p >/dev/null 2>&1; then
        echo 'xcode_command_line_tools=present'
        if /usr/bin/xcrun --find clang >/dev/null 2>&1; then
            echo 'clang=present'
        else
            echo 'clang=missing'
        fi
    else
        echo 'xcode_command_line_tools=missing'
        echo 'clang=not_checked'
    fi

    # Finder launches do not always inherit the terminal's cargo PATH.
    rustup_bin="$(command -v rustup || true)"
    if [ -z "$rustup_bin" ] && [ -x "${HOME}/.cargo/bin/rustup" ]; then
        rustup_bin="${HOME}/.cargo/bin/rustup"
    fi
    if [ -n "$rustup_bin" ]; then
        echo 'rustup=present'
        # `rustup run` never installs a missing toolchain or updates rustup.
        if pinned_rust="$("$rustup_bin" run 1.98.1 rustc --version 2>/dev/null)"; then
            echo "pinned_rust=$pinned_rust"
        else
            echo 'pinned_rust=missing'
        fi
    else
        echo 'rustup=missing'
        echo 'pinned_rust=not_checked'
    fi

    # Operate only on new synthetic scratch files. This is not a durability test.
    printf 'synthetic macOS readiness data\n' > "$work/source.tmp"
    /bin/cp "$work/source.tmp" "$work/copy.tmp"
    if /usr/bin/cmp -s "$work/source.tmp" "$work/copy.tmp"; then
        echo 'scratch_copy_compare=pass'
    else
        echo 'scratch_copy_compare=fail'
    fi
    /bin/ln "$work/source.tmp" "$work/hardlink.tmp"
    echo "scratch_hardlink_count=$(/usr/bin/stat -f '%l' "$work/source.tmp")"
    /bin/ln -s 'source.tmp' "$work/symlink.tmp"
    if [ -L "$work/symlink.tmp" ]; then
        echo 'scratch_symlink=pass'
    else
        echo 'scratch_symlink=fail'
    fi
    echo 'native_cryptex_tests=not_run'
    echo 'secure_store_tests=not_run'
    echo 'power_loss_tests=not_run'
    echo 'publication_enabled=false'
} > "$report"

/usr/bin/zip -j -q "$result" "$report"
echo "Kontrol tamamlandi. MAC-SONUC.zip dosyasini sohbete ekle."
/usr/bin/open -R "$result"
