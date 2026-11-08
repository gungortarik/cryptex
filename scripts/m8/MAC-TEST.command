#!/bin/bash
# Build pinned source on the Mac and collect narrowly scoped native test logs.
set -euo pipefail
umask 077

if [ "$(uname -s)" != 'Darwin' ]; then
    echo 'Bu paket gercek Mac uzerinde calistirilmalidir.'
    exit 2
fi

mode=full
if [ "$#" -eq 1 ] && [ "$1" = '--lint-only' ]; then
    mode=lint
elif [ "$#" -ne 0 ]; then
    echo 'Gecersiz test secenegi.'
    exit 2
fi

base="$(cd "$(dirname "$0")" && pwd)"
work="$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/cryptex-mac-test.XXXXXX")"
report="$work/result.txt"
result="$work/MAC-TEST-SONUC.zip"
if [ "$mode" = lint ]; then result="$work/MAC-LINT-SONUC.zip"; fi
{
    echo 'schema=1'
    if [ "$mode" = lint ]; then
        echo 'scope=macos_native_compile_and_clippy_only'
        echo 'probe_id=m8-macos-lint-20261108-v1'
        echo 'native_tests=not_rerun'
    else
        echo 'scope=macos_native_primitive_validation_and_keychain_tests'
        echo 'probe_id=m8-macos-native-20261108-v1'
    fi
    echo "collected_utc=$(/bin/date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "macos_version=$(/usr/bin/sw_vers -productVersion)"
    echo "macos_build=$(/usr/bin/sw_vers -buildVersion)"
    echo "architecture=$(/usr/bin/uname -m)"
    echo 'publication_enabled=false'
    echo 'windows_publication_service=not_available_on_macos'
    echo 'managed_endpoint_ownership=not_tested'
    echo 'power_loss_tests=not_run'
    echo 'independent_security_review=not_performed'
} > "$report"

finish() {
    runner_code=$?
    trap - EXIT
    set +e
    echo "runner_exit=$runner_code" >> "$report"
    files=(result.txt)
    for name in build.log instance.log staging.log incoming.log trusted.log keychain.log clippy.log; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$result" "${files[@]}"); then
        echo "${result##*/} hazir. Sonuc basarili olsa da hata olsa da bu ZIP dosyasini sohbete ekle."
        /usr/bin/open -R "$result"
    else
        echo "Sonuc ZIP dosyasi olusturulamadi. Rapor burada: $report"
    fi
    exit "$runner_code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

commit="$(/bin/cat "$base/SOURCE-COMMIT.txt")"
expected="$(/bin/cat "$base/SOURCE-SHA256.txt")"
if [ "${#commit}" -ne 40 ] || [[ "$commit" == *[!0-9a-f]* ]] ||
   [ "${#expected}" -ne 64 ] || [[ "$expected" == *[!0-9a-f]* ]]; then
    echo 'outcome=invalid_package_binding' >> "$report"
    exit 2
fi
actual="$(/usr/bin/shasum -a 256 "$base/source.zip" | /usr/bin/awk '{print $1}')"
echo "source_commit=$commit" >> "$report"
echo "source_sha256=$actual" >> "$report"
if [ "$actual" != "$expected" ]; then
    echo 'outcome=source_hash_mismatch' >> "$report"
    exit 2
fi

rustup_bin="$(command -v rustup || true)"
if [ -z "$rustup_bin" ] && [ -x "${HOME}/.cargo/bin/rustup" ]; then
    rustup_bin="${HOME}/.cargo/bin/rustup"
fi
if [ -n "$rustup_bin" ] && pinned_version="$("$rustup_bin" run 1.98.1 rustc --version 2>/dev/null)"; then
    echo "pinned_rust=$pinned_version" >> "$report"
else
    echo 'outcome=pinned_toolchain_missing' >> "$report"
    exit 2
fi

mkdir "$work/source"
/usr/bin/unzip -q "$base/source.zip" -d "$work/source"
cd "$work/source"
export CARGO_TARGET_DIR="$work/target"
echo 'Kaynak Mac uzerinde derleniyor. Ilk derlemede kilitli Cargo bagimliliklari indirilebilir; bu kisim uzun surebilir.'
if "$rustup_bin" run 1.98.1 cargo test --locked --color never -p cryptexd --lib --no-run > "$work/build.log" 2>&1; then
    echo 'build=pass' >> "$report"
else
    code=$?
    echo "build=failed:$code" >> "$report"
    echo 'outcome=build_failed' >> "$report"
    exit 2
fi

failures=0
run_tests() {
    name="$1"
    filter="$2"
    echo "Test: $name"
    if "$rustup_bin" run 1.98.1 cargo test --offline --locked --color never -p cryptexd --lib "$filter" -- --test-threads=1 > "$work/$name.log" 2>&1; then
        summary="$(/usr/bin/awk '/^test result:/{line=$0} END{print line}' "$work/$name.log")"
        case "$summary" in
            'test result: ok. 0 passed;'*) status='failed_no_matching_tests'; failures=$((failures + 1)) ;;
            'test result: ok.'*) status='pass' ;;
            *) status='failed_missing_summary'; failures=$((failures + 1)) ;;
        esac
        echo "${name}_summary=$summary" >> "$report"
    else
        code=$?
        status="failed:$code"
        failures=$((failures + 1))
    fi
    echo "$name=$status" >> "$report"
}

if [ "$mode" = full ]; then
    run_tests instance 'instance::tests::'
    run_tests staging 'import_staging::tests::'
    run_tests incoming 'incoming_validation::tests::'
    run_tests trusted 'trusted_import_probe::tests::'
    echo 'Keychain testi yalnizca sentetik gelistirme test kayitlarini kullanir. macOS izin penceresi gosterebilir.'
    echo 'secure_store_tests=attempted' >> "$report"
    run_tests keychain 'secure_store::macos::tests::'
fi

echo 'Mac hedefinde Clippy kontrolu...'
if "$rustup_bin" run 1.98.1 cargo clippy --offline --locked --color never -p cryptexd --all-targets -- -D warnings > "$work/clippy.log" 2>&1; then
    echo 'clippy=pass' >> "$report"
else
    code=$?
    echo "clippy=failed_or_unavailable:$code" >> "$report"
    failures=$((failures + 1))
fi
echo "failed_checks=$failures" >> "$report"
if [ "$failures" -eq 0 ]; then
    if [ "$mode" = lint ]; then
        echo 'outcome=compile_and_clippy_passed' >> "$report"
    else
        echo 'outcome=selected_native_checks_passed' >> "$report"
    fi
else
    if [ "$mode" = lint ]; then
        echo 'outcome=compile_and_clippy_failed' >> "$report"
    else
        echo 'outcome=selected_native_checks_failed' >> "$report"
    fi
    exit 2
fi
