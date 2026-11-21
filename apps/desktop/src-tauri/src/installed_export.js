(async () => {
  const requestedMode = "EXPECTED_MODE";
  const mode = ({ "writer-reenable": "writer-enable", "writer-next": "writer-resume",
    "writer-verify-next": "writer-verify" })[requestedMode] ?? requestedMode;
  const writer = mode.startsWith("writer-");
  if (mode.startsWith("first-device-")) {
    const deadline = Date.now() + 60000;
    let clicked = false;
    let visible = false;
    try {
      while (Date.now() < deadline) {
        const section = document.querySelector("[data-first-device-recovery]");
        if (section) {
          section.open = true;
          const label = mode.startsWith("first-device-check") ? "Check Kit and backup" :
            ["first-device-clean-prepare", "first-device-clean-reject"].includes(mode) ? "Prepare replacement profile" :
            mode.endsWith("-export") ? "Save public request" : "Import signed approval";
          const button = [...section.querySelectorAll("button")].find(el => el.textContent === label);
          if (!clicked && button && !button.disabled) { clicked = true; button.click(); }
          const status = section.querySelector('[role="status"]')?.textContent;
          if (status) {
            visible = mode === "first-device-check" ? status === "Recovery Kit and backup verified." &&
              section.querySelectorAll("dd").length === 3 :
              mode === "first-device-clean-prepare" ? status.startsWith("Replacement profile prepared.") :
              ["first-device-check-damaged", "first-device-clean-reject"].includes(mode) ? status.startsWith("Cryptex could not verify") :
              mode.endsWith("-export") ? status === "Public recovery request saved." &&
              ["Recovery Kit ID", "Backup SHA-256", "New key fingerprint · SHA-256"].every(label =>
                [...section.querySelectorAll("dt")].some(term => term.textContent === label &&
                  /^[0-9a-f]{64}$/.test(term.nextElementSibling?.textContent ?? ""))) :
              mode === "first-device-wrong" ? status.startsWith("Cryptex could not verify") :
              status === "Signed approval verified for this device.";
            break;
          }
        }
        await new Promise(resolve => setTimeout(resolve, 50));
      }
    } finally {
      await window.__TAURI_INTERNALS__.invoke("m11_search_observed", {visible});
    }
    return;
  }
  const panel = () => document.querySelector(writer ? '[aria-labelledby="backup-writer-heading"]' : '[aria-labelledby="manual-backup-heading"]');
  const transfer = {
    "writer-transfer-share": ["Share current assignment", /^Saved manual-[0-9a-f]{32}\.writer-assignment\.json in the selected backup target\.$/],
    "writer-transfer-request": ["Request writer role", /^Saved manual-[0-9a-f]{32}\.writer-request\.json in the selected backup target\.$/],
    "writer-transfer-sign": ["Review writer transfer", /^Saved manual-[0-9a-f]{32}\.writer-grant\.json in the selected backup target\.$/],
    "writer-transfer-cancel": ["Review writer transfer", /^Selection cancelled\.$/],
    "writer-transfer-accept": ["Accept signed grant", /^This device now holds the writer role for the selected target\.$/],
    "writer-transfer-peer": ["Check writer", /^This device is the designated writer for the selected target\.$/],
    "writer-transfer-former": ["Check writer", /^This device could not prove writer authority for the selected target\.$/],
  }[mode];
  const action = transfer?.[0] ?? (mode.startsWith("writer-verify") ? "Verify scheduled backup" : mode === "writer-retain" ? "Save retention" : mode === "writer-enable" ? "Enable daily backup" : mode === "writer-pause" ? "Pause scheduled backups" : writer ? (["writer-create", "writer-decline"].includes(mode) ? "Create backup target" : "Check writer") :
    (mode.startsWith("verify") ? "Verify manual backup" : "Export encrypted backup"));
  let clicked = false;
  let visible = false;
  try {
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      if (transfer && panel()?.querySelector("details:not([open])")) panel().querySelector("summary").click();
      const button = [...(panel()?.querySelectorAll("button") ?? [])].find(el =>
        el.textContent === action);
      if (!clicked && button && !button.disabled) {
        clicked = true;
        button.click();
      }
      const status = panel()?.querySelector('[role="status"]')?.textContent;
      visible = transfer ? transfer[1].test(status ?? "") : mode === "writer-create" ? /^Assigned this device as writer for cryptex-backup-[0-9a-f]{64}\.$/.test(status ?? "") :
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
