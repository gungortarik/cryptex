//! Reopen the accepted synthetic Brain through the regular guarded note reader.
use super::{DEV, LifecycleError, fault, provenance::Source};
use cryptex_core::DeviceId;
use cryptex_projection::search::TimelineQuery;
use cryptexd::{
    SecureStore,
    knowledge_materialization::{
        rebuild_knowledge_from_canonical, timeline_events_with_retrieval_safety_deadline,
    },
    local_device_identity::LocalDeviceKeyResolver,
    projection::{open_projection_db, rebuild_device_projection},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub(super) fn open(
    root: &Path,
    store: &impl SecureStore,
    device: &DeviceId,
    brain: &str,
    sources: Vec<Source>,
) -> Result<Value, LifecycleError> {
    let mut expected = BTreeMap::new();
    for source in sources {
        if expected.insert(source.event_id.clone(), source).is_some() {
            return Err(fault("duplicate_source_event"));
        }
    }
    let count = expected.len();
    let mut resolver = LocalDeviceKeyResolver::load(root, brain, store, DEV)
        .map_err(|_| fault("cutover_key_unavailable"))?;
    let (mut db, _) =
        open_projection_db(root).map_err(|_| fault("cutover_projection_unavailable"))?;
    rebuild_device_projection(store, DEV, root, device, &db)
        .map_err(|_| fault("cutover_note_rebuild_failed"))?;
    let devices = std::slice::from_ref(device);
    rebuild_knowledge_from_canonical(root, brain, devices, &mut resolver, &mut db)
        .map_err(|_| fault("cutover_knowledge_rebuild_failed"))?;
    let mut query = TimelineQuery {
        limit: 50,
        ..Default::default()
    };
    loop {
        let page = timeline_events_with_retrieval_safety_deadline(
            root,
            brain,
            devices,
            &mut resolver,
            &db,
            &query,
            None,
        )
        .map_err(|_| fault("cutover_guarded_read_failed"))?;
        for hit in page.hits {
            let source = expected
                .remove(&hit.event_id)
                .ok_or_else(|| fault("cutover_unexpected_note"))?;
            if hit.content_hash != source.content_hash
                || hit.classification != source.approval.classification.as_str()
                || hit.brain_id != brain
                || hit.device_id != device.as_str()
                || format!("{:x}", Sha256::digest(hit.text.as_bytes()))
                    != source.approval.source_sha256
            {
                return Err(fault("cutover_note_mismatch"));
            }
        }
        if page.next_cursor.is_none() {
            break;
        }
        query.cursor = page.next_cursor;
    }
    if !expected.is_empty() {
        return Err(fault("cutover_missing_notes"));
    }
    Ok(
        json!({"schema":"cryptex.m15.disposable-cutover.v1","brain_id":brain,"verified_notes":count}),
    )
}
