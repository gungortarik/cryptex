"""Verify live peers from a sealed installed Mac bundle in a disposable hosted account."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from update_fixture import ROOT, image, receipt, signer

from prepare_mac_inventory import prepare, stage_delivery, write_inventory
from build_owner_mac_package import fingerprint, package as package_for_owner


def run(*args, **kwargs):
    return subprocess.run(list(args), check=True, **kwargs)


def exercise(root, revision):
    config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.bundle.conf.json").read_text())
    config["bundle"]["resources"]["../dist"] = "frontend"
    overlay = root / "bundle.json"
    overlay.write_text(json.dumps(config), encoding="utf-8")
    profile_trial = os.environ.get("M17_MAC_TRIAL") == "persistent"
    feature = "m17-sealed-profile-trial" if profile_trial else "m17-sealed-startup-check"
    run("node", "--input-type=module", "-e",
        "import {prepareSidecar} from './scripts/prepare-cryptexd-sidecar.mjs'; "
        "prepareSidecar(undefined,process.env.M17_MAC_TRIAL === 'persistent' ? 'profile-trial' : true)",
        cwd=ROOT / "apps/desktop", timeout=1200)
    run("pnpm", "--filter", "@cryptex/desktop", "exec", "tauri", "build", "--bundles", "app",
        "--features", feature, "--config", overlay, cwd=ROOT, timeout=1200)
    if profile_trial:
        from mac_ci_credential_keychain import require_hosted_mac
        require_hosted_mac()
        home = Path.home()
        installed = home / "Applications/Cryptex.app"
        state = home / "Library/Application Support/Cryptex-Updater"
        if (home != home.resolve(strict=True) or installed.parent != installed.parent.resolve()
                or state != state.resolve() or any(os.path.lexists(path) for path in
                (installed, installed.with_suffix(".app.pending"), installed.with_suffix(".app.previous"), state))):
            raise RuntimeError("Disposable account install or updater slot is occupied or indirect")
        installed.parent.mkdir(exist_ok=True)
    else:
        installed = root / "Applications/Cryptex.app"
        installed.parent.mkdir()
    unsigned = ROOT / "target/release/bundle/macos/Cryptex.app"
    transport_features = ["--features", "owner-transport-update"] if profile_trial else []
    run("cargo", "test", "--locked", "-p", "cryptex-updater", *transport_features, cwd=ROOT, timeout=900)
    run("cargo", "build", "--locked", "--release", "-p", "cryptex-updater", *transport_features,
        cwd=ROOT, timeout=900)
    updater = ROOT / "target/release/cryptex-updater"
    image(updater, "com.cryptex.updater")
    public_der = Path(os.environ["CRYPTEX_M17_SIGNER_DER"])
    pin = hashlib.sha256(public_der.read_bytes()).hexdigest()
    helper = Path(os.environ["CX_UPDATE_CI_CMS_HELPER"])
    delivery = root / "signed-mac-delivery"
    approved_input = fingerprint(updater, unsigned)
    try:
        package_for_owner(unsigned, "0" * 64, revision, public_der, pin, updater, helper, delivery)
    except ValueError as error:
        if "Different unsigned application" not in str(error):
            raise
    else:
        raise RuntimeError("Signed an unapproved application")
    if delivery.exists():
        raise RuntimeError("Rejected signing request left a delivery")
    try:
        package_for_owner(unsigned, approved_input, "0" * 40, public_der, pin, updater, helper, delivery)
    except ValueError as error:
        if "Mac build provenance differs" not in str(error):
            raise
    else:
        raise RuntimeError("Owner signing accepted a different source revision")
    if delivery.exists():
        raise RuntimeError("Wrong revision published a Mac delivery")
    signed = package_for_owner(unsigned, approved_input, revision, public_der, pin, updater, helper,
                               delivery, os.environ["CX_NATIVE_TEST_KEYCHAIN"])
    manifest = json.loads((delivery / "release.json").read_bytes())
    if manifest != signed or manifest["version"] != "1.0.2" or manifest["platform"] != "macos":
        raise RuntimeError("Owner delivery record differs from the approved candidate version")
    if fingerprint(updater, unsigned) != approved_input:
        raise RuntimeError("Owner signing changed the approved build input")
    run("/usr/bin/ditto", delivery / "Cryptex.app", installed)
    desktop = installed / "Contents/MacOS/cryptex-desktop"
    daemon = desktop.with_name("cryptexd")
    run("/usr/bin/codesign", "--verify", "--strict", "--all-architectures", installed)
    run(sys.executable, "-B", ROOT / "scripts/m17/verify_first_install_mac.py", installed,
        pin, hashlib.sha256(desktop.read_bytes()).hexdigest())
    def slot():
        return installed.parent / ("cryptex-release-" + hashlib.sha256(desktop.read_bytes()).hexdigest())

    def publish():
        digest = hashlib.sha256(desktop.read_bytes()).hexdigest()
        result = prepare(installed, revision, pin, digest, helper, installed.parent)
        if result != slot():
            raise RuntimeError("Unexpected signed inventory slot")

    def startup(case, refusal=None):
        runtime = root / case
        result = subprocess.run([desktop, "--cryptex-m8-local-startup-check", runtime],
                                capture_output=True, text=True, timeout=90)
        if refusal:
            if (result.returncode != 2 or refusal not in result.stderr.splitlines()
                    or "=passed" in result.stdout or (runtime.exists() and any(runtime.iterdir()))):
                raise RuntimeError(f"{case} did not produce the expected refusal: {result.returncode} {result.stderr[-1024:]}")
        else:
            if result.returncode:
                raise RuntimeError(f"{case} refused signed startup: {result.returncode} {result.stderr[-1024:]}")
            for check in ("startup", "restart", "old_channel_refused", "cleanup"):
                if f"local_signed_{check}=passed" not in result.stdout.splitlines():
                    raise RuntimeError(f"Missing installed {check} evidence")

    try:
        prepare(installed, revision, pin, hashlib.sha256(desktop.read_bytes()).hexdigest(), helper, daemon.parent)
    except ValueError as error:
        if "outside the sealed application" not in str(error):
            raise
    else:
        raise RuntimeError("Signer accepted an output directory inside the sealed application")
    try:
        prepare(installed, "0" * 40, pin, hashlib.sha256(desktop.read_bytes()).hexdigest(), helper, installed.parent)
    except subprocess.CalledProcessError as error:
        if error.returncode != 2 or b"local_signed_startup_Inventory" not in (error.stderr or b"").splitlines():
            raise
    else:
        raise RuntimeError("A valid signature with the wrong source revision was published")
    if slot().exists():
        raise RuntimeError("Wrong revision occupied the approved slot")
    stage_delivery(installed, delivery / slot().name, pin, signed["desktop_sha256"], installed.parent)
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    try:
        publish()
    except FileExistsError:
        pass
    else:
        raise RuntimeError("Existing inventory was overwritten")
    startup("connected")
    if profile_trial:
        from mac_sealed_profile import check
        previous_daemon = Path(os.environ["RUNNER_TEMP"]) / "m17-previous/cryptexd"
        image(previous_daemon, "com.cryptex.daemon")
        check(root, desktop, pin, previous_daemon)
        print("installed_102_reopened_101_note_after_disposable_keychain_grant=passed", flush=True)
        from mac_update_credential import exercise as credential_update
        check(root, desktop, pin, daemon, lambda scope, profile:
              credential_update(root, desktop, updater, delivery, revision, pin, scope, profile))
        from mac_installed_transport import exercise as paired_update
        paired_update(root, desktop, updater, pin)
        return
    window = subprocess.run(["/usr/bin/swift", ROOT / "scripts/m8/mac_ui_quit_check.swift", desktop],
                            capture_output=True, text=True, timeout=120)
    if window.returncode or "sealed_window_verified_note_and_cleanup=passed" not in window.stdout.splitlines():
        raise RuntimeError(f"Ordinary window launch failed its receipt check: {window.returncode} {window.stderr[-1024:]}")
    refused = subprocess.run([desktop, "--cryptex-m8-local-content-existing-profile-ui", root / "unused"],
                             capture_output=True, text=True, timeout=30)
    if refused.returncode != 2 or "sealed_rehearsal_arguments" not in refused.stderr.splitlines() or (root / "unused").exists():
        raise RuntimeError("Sealed rehearsal accepted an existing-profile launch")
    original_slot = slot()
    previous = installed.with_suffix(".app.previous")
    installed.rename(previous)
    run("/usr/bin/ditto", previous, installed)
    notice = installed / "Contents/Resources/notices/frontend.json"
    with notice.open("ab") as out:
        out.write(b"\n")
    image(installed, "com.cryptex.desktop")
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    candidate_slot = slot()
    if candidate_slot == original_slot:
        raise RuntimeError("Sealed resource change retained the previous release identity")
    # Both images have the same source revision, publisher and roles.
    candidate_slot.mkdir()
    for name in ("inventory.json", "inventory.cms"):
        shutil.copyfile(original_slot / name, candidate_slot / name)
    startup("stale-inventory", "local_signed_startup_Inventory")
    candidate_slot.rename(root / "stale-metadata")
    publish()
    startup("new-release")
    installed.rename(installed.with_suffix(".app.pending"))
    previous.rename(installed)
    if slot() != original_slot:
        raise RuntimeError("Rollback did not select the original inventory")
    startup("restored-release")
    pending = installed.with_suffix(".app.pending")
    isolated = dict(os.environ, HOME=str(root))
    approved = {}
    for label, package in (("original", installed), ("candidate", pending)):
        statement = root / f"{label}.cms"
        if label == "original":
            shutil.copyfile(delivery / "update.cms", statement)
        else:
            receipt(updater, package, statement)
        run(updater, "admit", package, statement, env=isolated, timeout=90)
        approved[label] = run(updater, "fingerprint", package, capture_output=True, text=True).stdout.strip()

    def apply(expected_outcome, selected, expected_checks):
        process = run(updater, "apply", approved["candidate"], stdout=subprocess.PIPE,
                      text=True, env=isolated, timeout=150)
        report = json.loads(process.stdout)
        if (report["outcome"] != expected_outcome or len(report["checks"]) != expected_checks
                or any(check["status"] != "connected" for check in report["checks"])):
            raise RuntimeError(f"Unexpected signed update result: {report}")
        if run(updater, "fingerprint", installed, capture_output=True, text=True).stdout.strip() != approved[selected]:
            raise RuntimeError("Updater selected an unexpected installed image")
        run("/usr/bin/swift", ROOT / "scripts/m17/mac_reopened_window.swift", desktop,
            str(report["reopened_pid"]), timeout=90)
        print(f"sealed_updater={expected_outcome} verified_reopened_image={selected}", flush=True)

    # The updater has valid owner receipts for both images. Only the detached
    # live-peer inventory is missing when the candidate first becomes active.
    held = root / "held-inventory"
    candidate_slot.rename(held)
    apply("rolled_back_failed_probe", "original", 3)
    startup("updater-restored")
    distribution = root / "delivery"
    distribution.mkdir()
    candidate = distribution / "Cryptex.app"
    run("/usr/bin/ditto", pending, candidate)
    candidate_digest = hashlib.sha256((candidate / "Contents/MacOS/cryptex-desktop").read_bytes()).hexdigest()
    damaged = distribution / "damaged"
    damaged.mkdir()
    shutil.copyfile(held / "inventory.json", damaged / "inventory.json")
    (damaged / "inventory.cms").write_bytes(b"invalid")
    for foreign in (original_slot, damaged):
        try:
            stage_delivery(candidate, foreign, pin, candidate_digest, installed.parent)
        except subprocess.CalledProcessError as error:
            if error.returncode != 2 or b"local_signed_startup_Inventory" not in (error.stderr or b"").splitlines():
                raise
        else:
            raise RuntimeError("Receiving Mac staged invalid inventory")
        if candidate_slot.exists():
            raise RuntimeError("Rejected inventory occupied the release slot")
    for _ in range(2):
        if stage_delivery(candidate, held, pin, candidate_digest, installed.parent) != candidate_slot:
            raise RuntimeError("Receiving Mac selected the wrong release slot")
    apply("updated", "candidate", 2)
    startup("updater-selected")
    image(daemon, "com.cryptex.unrelated")
    image(installed, "com.cryptex.desktop")
    run("/usr/bin/codesign", "--verify", "--strict", installed)
    # The owner preparation command must refuse this daemon before publication.
    try:
        publish()
    except subprocess.CalledProcessError:
        pass
    else:
        raise RuntimeError("Owner inventory signer accepted an unrelated daemon")
    if slot().exists():
        raise RuntimeError("Rejected owner preparation published an inventory")
    # Independently exercise the receive-side check with a deliberately signed fixture.
    slot().mkdir()
    write_inventory(installed, revision, pin, helper, slot())
    startup("replaced-role", "local_signed_startup_Image")
    print("Verified sealed installation, live startup/restart, stale channel refusal, protected UI,"
          " stale inventory refusal and replaced role refusal.")


if __name__ == "__main__":
    with signer(), tempfile.TemporaryDirectory() as work:
        revision = os.environ["GITHUB_SHA"]
        os.environ["CRYPTEX_M17_SIGNER_DER"] = os.environ["CRYPTEX_UPDATE_SIGNER_DER"]
        os.environ["CRYPTEX_M17_SOURCE_REVISION"] = revision
        exercise(Path(work).resolve(), revision)
