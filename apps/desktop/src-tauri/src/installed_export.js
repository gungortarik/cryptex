(async () => {
  const mode = "EXPECTED_MODE";
  const writer = mode.startsWith("writer-");
  const panel = () => document.querySelector(writer ? '[aria-labelledby="backup-writer-heading"]' : '[aria-labelledby="manual-backup-heading"]');
  const action = mode.startsWith("writer-verify") ? "Verify scheduled backup" : mode === "writer-retain" ? "Save retention" : mode === "writer-enable" ? "Enable daily backup" : mode === "writer-pause" ? "Pause scheduled backups" : writer ? (["writer-create", "writer-decline"].includes(mode) ? "Create backup target" : "Check writer") :
    (mode.startsWith("verify") ? "Verify manual backup" : "Export encrypted backup");
  let clicked = false;
  let visible = false;
  try {
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      const button = [...(panel()?.querySelectorAll("button") ?? [])].find(el =>
        el.textContent === action);
      if (!clicked && button && !button.disabled) {
        clicked = true;
        button.click();
      }
      const status = panel()?.querySelector('[role="status"]')?.textContent;
      visible = mode === "writer-create" ? /^Assigned this device as writer for cryptex-backup-[0-9a-f]{64}\.$/.test(status ?? "") :
        mode === "writer-decline" ? status === "Selection cancelled." :
        mode === "writer-verify" ? status === "Scheduled backup verified by restoring it in temporary storage." :
        mode === "writer-verify-wrong" ? status?.startsWith("Cryptex could not complete scheduled backup verification") === true :
        mode === "writer-retain" ? status === "All scheduled backups will be kept." :
        mode === "writer-enable" ? status?.startsWith("Daily backup enabled.") === true :
        mode === "writer-pause" ? status === "Scheduled backups paused." :
        ["writer-inspect","writer-resume"].includes(mode) ? status === "This device is the designated writer for the selected target." :
        mode === "writer-copy" ? status === "This device could not prove writer authority for the selected target." :
        mode === "pair-write" ? /^Original backup copied as manual-[0-9a-f]{32}\.prior\.pgp\./.test(status ?? "") :
        mode === "verify-pair" ? status === "Original and new device backups both restored and verified." :
        mode === "verify" ? status === "Backup verified by restoring it in temporary storage." :
        mode === "verify-cancel" ? status === "Verification cancelled." :
        mode.startsWith("verify-") ? status?.startsWith("Verification failed.") === true :
        ["wrong-brain", "damaged"].includes(mode) ? status === "Cryptex could not verify the Recovery Kit certificate for this profile." :
        ["decline", "cancel"].includes(mode) ? status === "Export cancelled." :
        mode === "overlap" ? status === "Choose two separate folders outside the Cryptex profile." :
        /^Backup written as manual-[0-9a-f]{32}\.pgp\./.test(status ?? "");
      if (visible || status) break;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  } finally {
    await window.__TAURI_INTERNALS__.invoke("m11_search_observed", { visible });
  }
})();
