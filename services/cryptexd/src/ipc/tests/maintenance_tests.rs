use super::*;
use crate::managed_profile::connection_settings::{Error, Owner, State};
use public_consent::{Action, Outcome};

#[test]
fn maintenance_preserves_local_read_and_capture_and_restores_prior_ai_policy() {
    let mut fixture = prepared_search_fixture("maintenance");
    let instance = crate::instance::try_acquire_instance(&fixture.root).unwrap();
    fixture.context.admission = instance.admission();
    let mut owner = Owner::start(
        &fixture.root,
        &instance,
        fixture.store.clone(),
        fixture.context.environment,
        None,
        crate::approval_session::ApprovalSessionRegistry::default(),
        None,
    )
    .unwrap();
    fixture.context.connections = Some(owner.handle());
    let call = |operation: &str, update: serde_json::Value, authenticated| {
        let frame = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1, "requestId": "a1", "clientRole": "desktop", "operation": operation, "update": update
        })).unwrap();
        connection_settings_reply(&frame, Some(&fixture.context), authenticated)
    };
    let consent = &fixture.context.public_consent;
    for previously_paused in [true, false] {
        consent
            .decide(if previously_paused {
                Action::Pause
            } else {
                Action::Resume
            })
            .unwrap();
        assert_eq!(
            call(
                "connections.maintenance_enter",
                serde_json::Value::Null,
                false
            )
            .error,
            Some(Error::Unauthorized)
        );
        let change = |op| call(op, serde_json::Value::Null, true).view.unwrap().state;
        assert_eq!(change("connections.maintenance_enter"), State::Maintenance);
        assert_eq!(change("connections.read"), State::Maintenance);
        assert!(matches!(
            consent.decide(Action::Status),
            Ok(Outcome::Requests {
                paused: true,
                maintenance: true
            })
        ));
        assert!(consent.decide(Action::Resume).is_err());
        let update = serde_json::json!({"expectedRevision":"0","paused":false,"listenAddress":null,"peers":[]});
        assert_eq!(
            call("connections.update", update, true).error,
            Some(Error::PendingSettings)
        );
        let mut search = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        search.query.as_mut().unwrap().text = Some("synthetic".into());
        let read = handle_search(search, Some(&fixture.context), true);
        assert!(
            read.hits.as_ref().is_some_and(|hits| !hits.is_empty()),
            "Local read held: {:?}",
            read.hold_reason
        );
        let mut body = sample_capture_body();
        body.capture_id = new_bootstrap_event_id().unwrap();
        let capture = handle_capture_until(
            capture_request("desktop", vec![CAPABILITY_CONTENT_CAPTURE.into()], body),
            Some(&fixture.context),
            true,
        );
        assert_eq!(capture.outcome.as_deref(), Some("committed"));
        synchronize_local_derived_state(&fixture.context).unwrap();
        let mut captured_search = search_request("desktop", vec![CAPABILITY_CONTENT_SEARCH.into()]);
        captured_search.query.as_mut().unwrap().text = Some("inbox".into());
        let hits = handle_search(captured_search, Some(&fixture.context), true)
            .hits
            .unwrap();
        assert!(
            hits.iter()
                .any(|hit| Some(&hit.event_id) == capture.event_id.as_ref())
        );
        assert_eq!(change("connections.maintenance_leave"), State::NotEnrolled);
        assert!(
            matches!(consent.decide(Action::Status), Ok(Outcome::Requests { paused, maintenance: false }) if paused == previously_paused)
        );
    }
    owner.shutdown().unwrap();
    drop(instance);
    drop(fixture.context);
    fs::remove_dir_all(fixture.root).unwrap();
}
