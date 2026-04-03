use super::{NewWindowRequest, RespawnWindowRequest};
use crate::{SessionName, WindowTarget};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct OldNewWindowRequest {
    target: SessionName,
    name: Option<String>,
    detached: bool,
    environment: Option<Vec<String>>,
}

#[derive(Serialize)]
struct OldRespawnWindowRequest {
    target: WindowTarget,
    kill: bool,
    environment: Option<Vec<String>>,
}

fn session_name(value: &str) -> SessionName {
    SessionName::new(value).expect("valid session name")
}

#[test]
fn new_window_request_deserializes_old_payloads_with_defaulted_fields() {
    let bytes = bincode::serialize(&OldNewWindowRequest {
        target: session_name("alpha"),
        name: Some("logs".to_owned()),
        detached: true,
        environment: Some(vec!["FOO=1".to_owned()]),
    })
    .expect("old new-window request serializes");

    let decoded: NewWindowRequest =
        bincode::deserialize(&bytes).expect("new request decodes old payload");

    assert_eq!(decoded.target, session_name("alpha"));
    assert_eq!(decoded.name.as_deref(), Some("logs"));
    assert!(decoded.detached);
    assert_eq!(decoded.environment, Some(vec!["FOO=1".to_owned()]));
    assert_eq!(decoded.start_directory, None);
    assert_eq!(decoded.command, None);
    assert_eq!(decoded.target_window_index, None);
}

#[test]
fn respawn_window_request_deserializes_old_payloads_with_defaulted_fields() {
    let target = WindowTarget::with_window(session_name("alpha"), 2);
    let bytes = bincode::serialize(&OldRespawnWindowRequest {
        target: target.clone(),
        kill: true,
        environment: Some(vec!["FOO=1".to_owned()]),
    })
    .expect("old respawn-window request serializes");

    let decoded: RespawnWindowRequest =
        bincode::deserialize(&bytes).expect("new request decodes old payload");

    assert_eq!(decoded.target, target);
    assert!(decoded.kill);
    assert_eq!(decoded.environment, Some(vec!["FOO=1".to_owned()]));
    assert_eq!(decoded.start_directory, None);
    assert_eq!(decoded.command, None);
}

#[test]
fn new_and_respawn_window_requests_round_trip_with_spawn_fields() {
    let new_window = NewWindowRequest {
        target: session_name("alpha"),
        name: Some("logs".to_owned()),
        detached: true,
        start_directory: Some(PathBuf::from("/tmp/logs")),
        environment: Some(vec!["FOO=1".to_owned()]),
        command: Some(vec!["sleep".to_owned(), "30".to_owned()]),
        target_window_index: Some(5),
        insert_at_target: false,
    };
    let respawn_window = RespawnWindowRequest {
        target: WindowTarget::with_window(session_name("alpha"), 1),
        kill: true,
        start_directory: Some(PathBuf::from("/tmp/logs")),
        environment: Some(vec!["FOO=1".to_owned()]),
        command: Some(vec!["sleep".to_owned(), "30".to_owned()]),
    };

    assert_eq!(
        bincode::deserialize::<NewWindowRequest>(
            &bincode::serialize(&new_window).expect("new-window serializes")
        )
        .expect("new-window round-trips"),
        new_window
    );
    assert_eq!(
        bincode::deserialize::<RespawnWindowRequest>(
            &bincode::serialize(&respawn_window).expect("respawn-window serializes")
        )
        .expect("respawn-window round-trips"),
        respawn_window
    );
}
