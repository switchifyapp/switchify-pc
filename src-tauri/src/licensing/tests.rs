use super::*;
use activation::IdentityStore;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

struct FakeTransport {
    outcome: Mutex<&'static str>,
}
impl activation::Transport for FakeTransport {
    fn post(&self, _config: &activation::Config, body: Value) -> activation::ResponseFuture {
        let outcome = *self.outcome.lock().unwrap();
        Box::pin(async move {
            if outcome == "outage" {
                return Err(activation::NETWORK_ERROR.into());
            }
            if matches!(outcome, "seat_taken" | "takeover_limited" | "invalid") {
                return Ok(json!({"outcome":outcome}));
            }
            let request = &body["request"];
            if (now() - request["timestamp"].as_i64().unwrap()).abs() > 300 {
                return Err(activation::NETWORK_ERROR.into());
            }
            let terminal =
                matches!(outcome, "revoked" | "deactivated") || request["action"] == "deactivate";
            let server_time = now();
            let mut claims = json!({"version":1,"product":"switchify-pc","licence_id":request["licence_id"],
                    "installation_id":request["installation_id"],"installation_key":request["public_key"],
                    "generation":1,"issued_at":server_time,"request_nonce":request["nonce"],"action":request["action"]});
            if terminal {
                claims["status"] = json!(if outcome == "revoked" {
                    "revoked"
                } else {
                    "deactivated"
                });
            } else {
                claims["lease_id"] = json!(uuid::Uuid::new_v4().to_string());
                claims["expires_at"] = json!(server_time + TRIAL);
            }
            if outcome == "wrong_nonce" {
                claims["request_nonce"] = json!(uuid::Uuid::new_v4().to_string());
            }
            let message = format!(
                "{}.lease.{}",
                if terminal { "SWPS1" } else { "SWPL1" },
                URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
            );
            let signature = SigningKey::from_bytes(&[8; 32]).sign(message.as_bytes());
            Ok(
                json!({"proof":format!("{message}.{}",URL_SAFE_NO_PAD.encode(signature.to_bytes()))}),
            )
        })
    }
}
fn purchase() -> String {
    let message=format!("SWPC1.test.{}",URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({"version":1,"licence_id":"6af44000-17b7-4d87-ad1d-32e9a31b331a","product":"switchify-pc","issued_at":1000,"expires_at":null,"seats":1})).unwrap()));
    format!(
        "{message}.{}",
        URL_SAFE_NO_PAD.encode(
            SigningKey::from_bytes(&[7; 32])
                .sign(message.as_bytes())
                .to_bytes()
        )
    )
}
fn online_service() -> (Licensing, Arc<FakeTransport>, PathBuf) {
    let mut service = configured_service(Record::new(now()));
    let dir = std::env::temp_dir().join(format!(
        "switchify-activation-test-{}",
        uuid::Uuid::new_v4()
    ));
    service.path = dir.join("licence.json");
    service.keys = vec![(
        "test".into(),
        SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
    )];
    service.lease_keys = licence_lease::LeaseKeys::new(
        vec![(
            "lease".into(),
            SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
        )],
        &service.keys,
    )
    .unwrap();
    let transport = Arc::new(FakeTransport {
        outcome: Mutex::new("licensed"),
    });
    service.transport = transport.clone();
    (service, transport, dir)
}
#[tokio::test]
async fn unverifiable_request_guidance_does_not_replace_or_revoke_cached_access() {
    let (service, transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let before = service.data.lock().unwrap().record.clone();
    *transport.outcome.lock().unwrap() = "invalid";
    assert_eq!(
        service.check("check", None, false).await.unwrap_err(),
        activation::INVALID_REQUEST_ERROR
    );
    assert_eq!(service.view().status, "licensed");
    assert!(service.data.lock().unwrap().record == before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn rejected_initial_activation_preserves_trial_and_suggests_key_and_clock_checks() {
    let (service, transport, dir) = online_service();
    *transport.outcome.lock().unwrap() = "invalid";
    assert_eq!(
        service
            .check("activate", Some(&purchase()), false)
            .await
            .unwrap_err(),
        activation::INVALID_REQUEST_ERROR
    );
    assert_eq!(service.view().status, "trial");
    assert!(service
        .data
        .lock()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .key
        .is_none());
    std::fs::remove_dir_all(dir).ok();
}
fn restart(source: &Licensing, record: Record) -> Licensing {
    let mut service = configured_service(record);
    service.keys = source.keys.clone();
    service.lease_keys = source.lease_keys.clone();
    service.path = source.path.clone();
    service.identity = source.identity.clone();
    service.mirror = source.mirror.clone();
    service.transport = source.transport.clone();
    service.seed_access();
    service
}
#[tokio::test]
async fn file_only_transplant_never_grants_paid_access_even_after_seat_conflict() {
    let (source, transport, dir) = online_service();
    source.data.lock().unwrap().record = Some(Record::new(0));
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let record = decode_record(&std::fs::read_to_string(&source.path).unwrap()).unwrap();
    let mut copied = restart(&source, record);
    copied.identity = Arc::new(activation::tests::OtherIdentity);
    copied.mirror = Arc::new(activation::tests::MemoryMirror::default());
    assert!(copied.view().locked, "file alone grants no startup access");
    copied.sync_mirror().await;
    assert_eq!(copied.view().status, "needs_activation");
    assert!(copied.view().locked);
    *transport.outcome.lock().unwrap() = "seat_taken";
    assert!(copied.check("check", None, false).await.is_err());
    assert!(copied.view().locked);
    assert!(copied
        .data
        .lock()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .lease
        .is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn cold_restart_waits_for_os_identity_then_recovers_offline_after_unlock() {
    let (source, transport, dir) = online_service();
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let record = decode_record(&std::fs::read_to_string(&source.path).unwrap()).unwrap();
    let mut cold = restart(&source, record.clone());
    cold.identity = Arc::new(LockedIdentity);
    *transport.outcome.lock().unwrap() = "outage";
    assert_eq!(cold.view().status, "verification_pending");
    assert!(cold.view().locked);
    cold.sync_mirror().await;
    assert!(cold.view().locked);
    assert!(cold.view().storage_warning);
    cold.data.lock().unwrap().clock.started =
        std::time::Instant::now() - std::time::Duration::from_secs(61);
    cold.checkpoint();
    cold.persist_file().await;
    let retained = decode_record(&std::fs::read_to_string(&cold.path).unwrap()).unwrap();
    assert!(
        retained == record,
        "locked-startup checkpoints retain the original seal and record"
    );
    assert!(cold.data.lock().unwrap().record.as_ref().unwrap().lease == record.lease);
    cold.identity = source.identity.clone();
    cold.data.lock().unwrap().next_secure_retry = 0;
    cold.sync_mirror().await;
    assert_eq!(cold.view().status, "licensed");
    assert!(!cold.view().storage_warning);
    assert!(cold
        .data
        .lock()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .authenticated(&cold.identity.load_or_create().unwrap()));
    assert!(cold.check("check", None, false).await.is_err());
    assert_eq!(
        cold.view().status,
        "licensed",
        "outage after verification keeps the allowance"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn locked_startup_checkpoint_cannot_restore_an_older_terminal_mirror() {
    let (source, transport, dir) = online_service();
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    *transport.outcome.lock().unwrap() = "deactivated";
    source.check("check", None, false).await.unwrap(); // Durable old mirror.
    *transport.outcome.lock().unwrap() = "licensed";
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let record = source.data.lock().unwrap().record.clone().unwrap();
    let mut cold = restart(&source, record.clone());
    cold.identity = Arc::new(LockedIdentity);
    cold.data.lock().unwrap().clock.started =
        std::time::Instant::now() - std::time::Duration::from_secs(61);
    cold.checkpoint();
    cold.persist_file().await;
    cold.identity = source.identity.clone();
    cold.sync_mirror().await;
    assert_eq!(cold.view().status, "licensed");
    assert_eq!(
        cold.data.lock().unwrap().record.as_ref().unwrap().lease,
        record.lease
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn authenticated_mirror_defeats_older_and_modified_revocation_rollbacks() {
    let (source, transport, dir) = online_service();
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let old = source.data.lock().unwrap().record.clone().unwrap();
    *transport.outcome.lock().unwrap() = "revoked";
    source.check("check", None, false).await.unwrap();
    source.sync_mirror().await;
    for tampered in [false, true] {
        let mut file = old.clone();
        if tampered {
            file.revision = u64::MAX;
            file.lease_seen = 0;
        }
        let cold = restart(&source, file);
        assert!(
            cold.view().locked,
            "no stale access before mirror reconciliation"
        );
        cold.sync_mirror().await;
        assert_eq!(cold.view().status, "revoked");
        assert!(cold.view().locked);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn frozen_and_rolled_back_wall_clock_cannot_stop_running_lease_expiry() {
    let (service, _transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    service.sync_mirror().await;
    assert!(!service.data.lock().unwrap().mirror_dirty);
    {
        let mut data = service.data.lock().unwrap();
        let issued = data.record.as_ref().unwrap().last_checked.unwrap();
        data.clock = OfflineClock {
            epoch: issued,
            started: std::time::Instant::now() - std::time::Duration::from_secs(TRIAL as u64),
        };
        assert!(data.clock.time(issued - DAY) >= issued + TRIAL);
    }
    assert_eq!(service.view().status, "lease_expired");
    assert!(!service.cached_allowed(now()));
    service.checkpoint();
    let record = service.data.lock().unwrap().record.clone().unwrap();
    assert!(record.authenticated(&service.identity.load_or_create().unwrap()));
    assert!(service.data.lock().unwrap().mirror_dirty);
    service.sync_mirror().await;
    let cold = restart(&service, record);
    cold.sync_mirror().await;
    assert_eq!(cold.view().status, "lease_expired");
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn same_lease_reconciliation_keeps_highest_authenticated_time_marker() {
    let (service, _transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let identity = service.identity.load_or_create().unwrap();
    let old = service.data.lock().unwrap().record.clone().unwrap();
    let mut newer = old.clone();
    newer.lease_seen += TRIAL;
    newer.seal(&identity);
    let reconciled = Record::reconcile(Some(old), Some(newer.clone()), &identity).unwrap();
    assert_eq!(reconciled.lease_seen, newer.lease_seen);
    assert!(reconciled.authenticated(&identity));
    assert_eq!(
        reconciled
            .view(&service.trust(), &service.lease_keys, now())
            .status,
        "lease_expired"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn unsigned_v2_cache_keeps_trial_and_purchase_for_online_recovery() {
    let (source, _transport, dir) = online_service();
    source.data.lock().unwrap().record = Some(Record::new(0));
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let mut legacy = source.data.lock().unwrap().record.clone().unwrap();
    legacy.version = 2;
    legacy.local_signature = None;
    let legacy = decode_record(&serde_json::to_string(&legacy).unwrap()).unwrap();
    let mut cold = restart(&source, legacy);
    cold.mirror = Arc::new(activation::tests::MemoryMirror::default());
    cold.sync_mirror().await;
    assert_eq!(cold.view().status, "needs_activation");
    assert!(cold.view().locked);
    assert_eq!(
        cold.data.lock().unwrap().record.as_ref().unwrap().first_run,
        0
    );
    cold.check("check", None, false).await.unwrap();
    assert_eq!(cold.view().status, "licensed");
    assert_eq!(
        cold.data.lock().unwrap().record.as_ref().unwrap().version,
        3
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn online_activation_outage_replay_refund_and_deactivation_use_one_access_gate() {
    let (service, transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    assert_eq!(service.view().status, "licensed");
    assert_eq!(service.view().offline_days_remaining, Some(30));
    let before = service.data.lock().unwrap().record.clone().unwrap();
    for failure in ["outage", "wrong_nonce"] {
        *transport.outcome.lock().unwrap() = failure;
        assert!(service.check("check", None, false).await.is_err());
        assert_eq!(service.view().status, "licensed");
        assert_eq!(
            service.data.lock().unwrap().record.as_ref().unwrap().lease,
            before.lease
        );
    }
    let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
    let expiry = before.last_checked.unwrap() + TRIAL;
    assert!(saved
        .view(&service.trust(), &service.lease_keys, expiry - 1)
        .allowed(expiry - 1));
    assert!(!saved
        .view(&service.trust(), &service.lease_keys, expiry)
        .allowed(expiry));
    *transport.outcome.lock().unwrap() = "revoked";
    service.check("check", None, false).await.unwrap();
    assert_eq!(service.view().status, "revoked");
    assert!(service.view().locked);
    {
        let mut data = service.data.lock().unwrap();
        assert!(
            data.cleanup_pending,
            "verified revocation latches cleanup before the next tick"
        );
        assert!(data.observe_access(false));
        assert!(data.cleanup_pending);
    }
    let revoked = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
    assert_eq!(
        revoked
            .view(&service.trust(), &service.lease_keys, now())
            .status,
        "revoked"
    );
    *transport.outcome.lock().unwrap() = "licensed";
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    service.check("deactivate", None, false).await.unwrap();
    assert_eq!(service.view().status, "trial");
    assert!(service
        .data
        .lock()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .key
        .is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn seat_conflict_preserves_trial_and_requires_explicit_transfer() {
    let (service, transport, dir) = online_service();
    let first_run = service
        .data
        .lock()
        .unwrap()
        .record
        .as_ref()
        .unwrap()
        .first_run;
    *transport.outcome.lock().unwrap() = "seat_taken";
    assert!(service
        .check("activate", Some(&purchase()), false)
        .await
        .is_err());
    assert!(service.view().can_transfer);
    assert_eq!(service.view().status, "trial");
    *transport.outcome.lock().unwrap() = "licensed";
    service.check("activate", None, true).await.unwrap();
    assert!(!service.view().can_transfer);
    assert_eq!(service.view().status, "licensed");
    assert_eq!(
        service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .first_run,
        first_run
    );
    std::fs::remove_dir_all(dir).unwrap();
}
struct LockedIdentity;
impl activation::IdentityStore for LockedIdentity {
    fn load_or_create(&self) -> Result<activation::Identity, String> {
        Err(activation::STORAGE_ERROR.into())
    }
}
#[tokio::test]
async fn cached_lease_opens_with_locked_keychain_and_mirror_heals_after_unlock() {
    let (mut service, _transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    service.identity = Arc::new(LockedIdentity);
    assert!(service.check("check", None, false).await.is_err());
    assert_eq!(service.view().status, "licensed");
    {
        let mut data = service.data.lock().unwrap();
        data.record.as_mut().unwrap().last_seen = now() - 61;
        data.storage_warning = true;
    }
    service.identity = Arc::new(activation::tests::FakeIdentity);
    service.checkpoint();
    service.sync_mirror().await;
    assert!(!service.view().storage_warning);
    service.check("check", None, false).await.unwrap();
    assert_eq!(service.view().status, "licensed");
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn verified_revocation_locks_current_session_even_when_record_file_cannot_be_written() {
    let (mut service, transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let old = service.data.lock().unwrap().record.clone().unwrap();
    service.path = dir.clone(); // A directory cannot be replaced by the record file.
    *transport.outcome.lock().unwrap() = "revoked";
    service.check("check", None, false).await.unwrap();
    assert!(service.view().locked);
    assert!(service.view().storage_warning);
    let cold = restart(&service, old);
    assert!(cold.view().locked);
    cold.sync_mirror().await;
    assert_eq!(
        cold.view().status,
        "revoked",
        "terminal mirror was durable before the command succeeded"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn modified_cache_without_mirror_requires_online_recovery() {
    let (source, _transport, dir) = online_service();
    source.data.lock().unwrap().record = Some(Record::new(0));
    source
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let mut modified = source.data.lock().unwrap().record.clone().unwrap();
    modified.revision = u64::MAX;
    modified.lease_seen = 0;
    let mut cold = restart(&source, modified);
    cold.mirror = Arc::new(activation::tests::MemoryMirror::default());
    cold.sync_mirror().await;
    assert!(cold.view().locked);
    assert_eq!(cold.view().status, "needs_activation");
    assert_eq!(
        cold.data.lock().unwrap().record.as_ref().unwrap().revision,
        0
    );
    cold.check("check", None, false).await.unwrap();
    assert_eq!(cold.view().status, "licensed");
    assert_eq!(
        cold.data.lock().unwrap().record.as_ref().unwrap().revision,
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn v1_record_migrates_without_resetting_trial_or_granting_purchase_only_access() {
    let old =
        json!({"version":1,"first_run":1000,"last_seen":1000+TRIAL,"revision":4,"key":purchase()});
    let record = decode_record(&old.to_string()).unwrap();
    assert_eq!(record.version, 3);
    assert_eq!(record.first_run, 1000);
    assert_eq!(record.revision, 4);
    let keys = [(
        "test",
        SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
    )];
    let view = record.view(&keys, &licence_lease::LeaseKeys::default(), 1000);
    assert_eq!(view.status, "needs_activation");
    assert!(view.locked);
    assert!(!view.allowed(1000));
}
#[tokio::test]
async fn corrected_clock_recovers_paid_access_without_resetting_trial_high_water() {
    let (service, _transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let future = now() + 60 * DAY;
    let stale = {
        let mut data = service.data.lock().unwrap();
        let record = data.record.as_mut().unwrap();
        record.last_seen = future;
        record.lease_seen = future;
        record.clone()
    };
    service
        .mirror
        .write(&serde_json::to_string(&stale).unwrap())
        .unwrap();
    assert!(service.view().locked);
    // The fake endpoint enforces the same +/-300-second request window as
    // Node. A trial high-water timestamp must never become wire freshness.
    service.check("check", None, false).await.unwrap();
    assert_eq!(service.view().status, "licensed");
    service.sync_mirror().await;
    assert_eq!(service.view().status, "licensed");
    assert_eq!(
        service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .last_seen,
        future
    );
    service.check("deactivate", None, false).await.unwrap();
    assert_eq!(
        service.view().status,
        "expired",
        "online recovery cannot restart the trial"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn unconfigured_build_does_not_start_a_persisted_trial() {
    let (mut service, _transport, dir) = online_service();
    service.configured = false;
    service.persist_file().await;
    service.sync_mirror().await;
    assert!(!service.path.exists());
    assert!(!dir.exists());
}
#[tokio::test]
async fn paid_recovery_from_unreadable_record_does_not_restart_trial() {
    let (service, _transport, dir) = online_service();
    service.data.lock().unwrap().record = None;
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    assert_eq!(service.view().status, "licensed");
    service.check("deactivate", None, false).await.unwrap();
    assert_eq!(service.view().status, "expired");
    assert_eq!(
        service
            .data
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .first_run,
        0
    );
    std::fs::remove_dir_all(dir).unwrap();
}
struct StalledMirror {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
    stored: Option<String>,
}
impl activation::MirrorStore for StalledMirror {
    fn read(&self) -> Result<Option<String>, String> {
        Ok(self.stored.clone())
    }
    fn write(&self, _text: &str) -> Result<(), String> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
            self.release
                .lock()
                .unwrap()
                .recv()
                .map_err(|_| "Mirror closed".to_string())?;
        }
        Ok(())
    }
}
#[tokio::test]
async fn stalled_keychain_never_holds_access_lock_or_delays_revocation_cleanup() {
    let (mut service, transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let mut old = service.data.lock().unwrap().record.clone().unwrap();
    old.revision = 5;
    old.first_run -= DAY;
    old.seal(&activation::tests::FakeIdentity.load_or_create().unwrap());
    std::fs::remove_file(&service.path).unwrap();
    {
        let mut data = service.data.lock().unwrap();
        data.record = Some(Record::new(now()));
        data.mirror_loaded = false;
    }
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service.mirror = Arc::new(StalledMirror {
        entered: Mutex::new(Some(entered_tx)),
        release: Mutex::new(release_rx),
        stored: Some(serde_json::to_string(&old).unwrap()),
    });
    let service = Arc::new(service);
    let worker = service.clone();
    let task = tokio::spawn(async move {
        worker.sync_mirror().await;
    });
    entered_rx.await.unwrap();
    assert!(
        service.data.try_lock().is_ok(),
        "keychain I/O holds no entitlement lock"
    );
    assert!(service.cached_allowed(now()));
    assert_eq!(service.view().status, "licensed");
    *transport.outcome.lock().unwrap() = "revoked";
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        service.check("check", None, false),
    )
    .await
    .unwrap()
    .unwrap_err(); // Saving is still pending, but revocation is immediate.
    assert!(service.view().locked);
    assert!(service.data.lock().unwrap().cleanup_pending);
    let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
    assert_eq!(saved.terminal_status.as_deref(), Some("revoked"));
    assert!(saved.revision > old.revision);
    assert_eq!(saved.first_run, old.first_run);
    release_tx.send(()).unwrap();
    task.await.unwrap();
    assert_eq!(
        service.view().status,
        "revoked",
        "a stale mirror completion cannot restore access"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
struct PausedReadMirror {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
    stored: String,
}
impl activation::MirrorStore for PausedReadMirror {
    fn read(&self) -> Result<Option<String>, String> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
        }
        self.release
            .lock()
            .unwrap()
            .recv()
            .map_err(|_| "Mirror closed")?;
        Ok(Some(self.stored.clone()))
    }
    fn write(&self, _text: &str) -> Result<(), String> {
        Ok(())
    }
}
#[tokio::test]
async fn delayed_initial_read_establishes_old_revision_before_fresh_server_commit() {
    let (mut service, transport, dir) = online_service();
    service
        .check("activate", Some(&purchase()), false)
        .await
        .unwrap();
    let mut old = service.data.lock().unwrap().record.clone().unwrap();
    old.revision = 5;
    old.first_run -= DAY;
    old.seal(&activation::tests::FakeIdentity.load_or_create().unwrap());
    std::fs::remove_file(&service.path).unwrap();
    {
        let mut data = service.data.lock().unwrap();
        data.record = Some(Record::new(now()));
        data.mirror_loaded = false;
    }
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service.mirror = Arc::new(PausedReadMirror {
        entered: Mutex::new(Some(entered_tx)),
        release: Mutex::new(release_rx),
        stored: serde_json::to_string(&old).unwrap(),
    });
    *transport.outcome.lock().unwrap() = "revoked";
    let service = Arc::new(service);
    let worker = service.clone();
    let task =
        tokio::spawn(async move { worker.check("activate", Some(&purchase()), false).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(service.data.try_lock().is_ok());
    assert_eq!(
        service.view().status,
        "trial",
        "cached launch state remains usable while keychain read waits"
    );
    assert!(
        !task.is_finished(),
        "network state cannot commit before the recovered revision is known"
    );
    release_tx.send(()).unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(service.view().status, "revoked");
    let saved = decode_record(&std::fs::read_to_string(&service.path).unwrap()).unwrap();
    assert!(saved.revision > old.revision);
    assert_eq!(saved.first_run, old.first_run);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn trial_uses_exact_boundary_and_never_restarts_on_rollback() {
    let record = Record::new(1000);
    assert_eq!(
        record
            .view(&[], &licence_lease::LeaseKeys::default(), 1000)
            .days_remaining,
        30
    );
    assert_eq!(
        record
            .view(&[], &licence_lease::LeaseKeys::default(), 1000)
            .message,
        "30 days left in your full trial."
    );
    assert_eq!(
        record
            .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL - 1)
            .message,
        "1 day left in your full trial."
    );
    assert!(record
        .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL - 1)
        .allowed(1000 + TRIAL - 1));
    assert!(!record
        .view(&[], &licence_lease::LeaseKeys::default(), 1000 + TRIAL)
        .allowed(1000 + TRIAL));
    let mut advanced = record.clone();
    advanced.last_seen = 1000 + TRIAL;
    assert_eq!(
        advanced
            .view(&[], &licence_lease::LeaseKeys::default(), 1000)
            .status,
        "expired"
    );
}
#[test]
fn mirror_reconciliation_preserves_trial_and_removal() {
    let old = Record {
        key: Some("old".into()),
        ..Record::new(1000)
    };
    let new = Record {
        key: None,
        revision: 1,
        ..Record::new(2000)
    };
    let merged = Record::merge(Some(new), Some(old)).unwrap();
    assert_eq!(merged.first_run, 1000);
    assert_eq!(merged.last_seen, 2000);
    assert!(merged.key.is_none());
}
#[test]
fn main_thread_observations_cannot_skip_cleanup_between_allowed_and_expired() {
    let mut data = Data {
        record: None,
        storage_warning: false,
        recovery: false,
        recovery_armed: false,
        cleanup_pending: false,
        last_allowed: true,
        access_until: Some(0),
        secure_failures: 0,
        next_secure_retry: 0,
        file_dirty: true,
        file_warning: false,
        mirror_dirty: true,
        mirror_loaded: false,
        identity: None,
        clock: OfflineClock::new(now()),
    };
    assert!(!data.observe_access(true));
    // An expiry or removal between callbacks cannot update last_allowed.
    assert!(data.observe_access(false));
    assert!(data.cleanup_pending);
    assert!(!data.observe_access(false));
    assert!(data.cleanup_pending); // failed cleanup stays pending
    data.cleanup_pending = false;
    assert!(!data.observe_access(true));
    assert!(data.observe_access(false));
}
fn configured_service(record: Record) -> Licensing {
    Licensing {
        data: Arc::new(Mutex::new(Data {
            record: Some(record),
            storage_warning: false,
            recovery: false,
            recovery_armed: false,
            cleanup_pending: false,
            last_allowed: true,
            access_until: Some(0),
            secure_failures: 0,
            next_secure_retry: 0,
            file_dirty: true,
            file_warning: false,
            mirror_dirty: true,
            mirror_loaded: false,
            identity: None,
            clock: OfflineClock::new(now()),
        })),
        path: std::env::temp_dir().join("switchify-licence-test-never-written.json"),
        keys: vec![("test".into(), [0; 32])],
        configured: true,
        lease_keys: licence_lease::LeaseKeys::new(vec![("lease".into(), [1; 32])], &[]).unwrap(),
        config: Some(activation::Config::test_config()),
        transport: Arc::new(activation::HttpTransport),
        identity: Arc::new(activation::tests::FakeIdentity),
        mirror: Arc::new(activation::tests::MemoryMirror::default()),
        operation: tokio::sync::Mutex::new(()),
        file_operation: tokio::sync::Mutex::new(()),
        mirror_operation: Arc::new(tokio::sync::Mutex::new(())),
        mirror_read_operation: tokio::sync::Mutex::new(()),
    }
}
#[test]
fn launching_an_expired_install_does_not_queue_cleanup() {
    let mut expired = Record::new(0);
    expired.last_seen = TRIAL;
    let service = configured_service(expired);
    service.seed_access();
    let mut data = service.data.lock().unwrap();
    assert!(!data.last_allowed);
    assert!(!data.observe_access(false));
    assert!(!data.cleanup_pending);
    drop(data);
    let service = configured_service(Record::new(now()));
    service.seed_access();
    let mut data = service.data.lock().unwrap();
    assert!(data.last_allowed);
    assert!(data.observe_access(false));
}
#[test]
fn cached_access_follows_the_last_computed_view() {
    let service = configured_service(Record::new(now()));
    service.view();
    assert!(service.cached_allowed(now()));
    let mut expired = Record::new(0);
    expired.last_seen = TRIAL;
    service.data.lock().unwrap().record = Some(expired);
    assert!(service.cached_allowed(now()), "stale until refreshed");
    service.view();
    assert!(!service.cached_allowed(now()));
}
#[test]
fn fresh_install_starts_the_trial_even_when_the_keychain_is_unavailable() {
    let fresh = initial_record(None, None, false, 5000).unwrap();
    assert_eq!(fresh.first_run, 5000);
    assert!(initial_record(None, None, true, 5000).is_none());
    let older = Record::new(1000);
    let kept = initial_record(Some(fresh), Some(older), false, 9000).unwrap();
    assert_eq!(
        kept.first_run, 1000,
        "the keychain's earlier trial wins once it is back"
    );
}
#[test]
fn unreadable_file_keeps_the_keychain_record_and_fails_closed_without_one() {
    let old = Record::new(1000);
    let kept = initial_record(None, Some(old.clone()), true, 9000).unwrap();
    assert_eq!(kept.first_run, 1000);
}
#[test]
fn stale_keychain_keeps_its_warning_and_backs_off_until_a_retry_succeeds() {
    let mut data = Data {
        record: None,
        storage_warning: true,
        recovery: false,
        recovery_armed: false,
        cleanup_pending: false,
        last_allowed: true,
        access_until: Some(0),
        secure_failures: 0,
        next_secure_retry: 0,
        file_dirty: true,
        file_warning: false,
        mirror_dirty: true,
        mirror_loaded: false,
        identity: None,
        clock: OfflineClock::new(now()),
    };
    // Retries at 60, then 60 + 120, then + 240 ... capped at 64 minutes.
    let mut time = 60;
    let mut expected = vec![];
    for _ in 0..8 {
        assert!(data.retry_secure_due(time));
        data.record_checkpoint(time, true, Ok(true));
        expected.push(time);
        // Non-retry minutes keep the warning and the schedule untouched.
        assert!(!data.retry_secure_due(time + 60));
        data.record_checkpoint(time + 60, false, Ok(false));
        assert!(data.storage_warning);
        time = data.next_secure_retry;
    }
    assert_eq!(expected, [60, 180, 420, 900, 1860, 3780, 7620, 11460]);
    // A file failure sets the warning without touching the keychain schedule.
    let scheduled = data.next_secure_retry;
    data.record_checkpoint(time, false, Err(()));
    assert!(data.storage_warning);
    assert_eq!(data.next_secure_retry, scheduled);
    // The first successful keychain retry clears everything.
    data.record_checkpoint(time, true, Ok(false));
    assert!(!data.storage_warning);
    assert_eq!(data.secure_failures, 0);
    assert!(!data.retry_secure_due(time + 60));
}
#[test]
fn activated_record_fits_the_windows_credential_store() {
    const MAX_TIME: i64 = 253_402_300_799;
    let id = "6af44000-17b7-4d87-ad1d-32e9a31b331a";
    let public_key = "x".repeat(43);
    let key_id = "x".repeat(32);
    let signature = "x".repeat(86);
    let signed = |prefix: &str, claims: Value| {
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        format!("{prefix}.{key_id}.{payload}.{signature}")
    };
    let key = signed(
        "SWPC1",
        json!({"version":1,"licence_id":id,"product":"switchify-pc","issued_at":MAX_TIME,
                "expires_at":MAX_TIME,"seats":u32::MAX}),
    );
    let lease = signed(
        "SWPL1",
        json!({"version":1,"product":"switchify-pc","licence_id":id,"installation_id":id,
                "installation_key":public_key,"generation":(1u64 << 53) - 1,"issued_at":MAX_TIME,
                "request_nonce":id,"action":"activate","lease_id":id,"expires_at":MAX_TIME}),
    );
    let record = Record {
        first_run: MAX_TIME,
        last_seen: MAX_TIME,
        lease_seen: MAX_TIME,
        revision: u64::MAX,
        pending_key: Some(key.clone()),
        key: Some(key),
        lease: Some(lease),
        installation: Some(activation::Installation {
            id: id.into(),
            public_key: public_key.clone(),
        }),
        generation: u64::MAX,
        last_checked: Some(MAX_TIME),
        last_outcome: Some("takeover_limited".into()),
        local_signature: Some(signature.clone()),
        ..Record::new(0)
    };
    let text = serde_json::to_string(&record).unwrap();
    assert!(text.len() <= activation::MIRROR_MAX_BYTES, "{}", text.len());
}
#[test]
fn stale_revocation_in_the_mirror_cannot_lock_a_reactivated_licence() {
    let identity = activation::tests::FakeIdentity.load_or_create().unwrap();
    let mut revoked = Record::new(1000);
    revoked.key = Some(purchase());
    revoked.installation = Some(identity.installation.clone());
    revoked.terminal_status = Some("revoked".into());
    revoked.revision = 3;
    revoked.seal(&identity);
    let mut activated = revoked.clone();
    activated.terminal_status = None;
    activated.lease = Some("lease".into());
    activated.revision = 5;
    activated.seal(&identity);
    let merged = Record::reconcile(Some(activated), Some(revoked), &identity).unwrap();
    assert_eq!(merged.terminal_status, None);
    assert_eq!(merged.lease.as_deref(), Some("lease"));
}
#[test]
fn stale_mirror_cannot_undo_a_newer_revocation_in_the_file() {
    let identity = activation::tests::FakeIdentity.load_or_create().unwrap();
    let mut licensed = Record::new(1000);
    licensed.key = Some(purchase());
    licensed.installation = Some(identity.installation.clone());
    licensed.lease = Some("lease".into());
    licensed.revision = 3;
    licensed.seal(&identity);
    let mut revoked = licensed.clone();
    revoked.lease = None;
    revoked.terminal_status = Some("revoked".into());
    revoked.revision = 5;
    revoked.seal(&identity);
    let merged = Record::reconcile(Some(revoked), Some(licensed), &identity).unwrap();
    assert_eq!(merged.terminal_status.as_deref(), Some("revoked"));
    assert!(merged.lease.is_none());
}
#[test]
fn legacy_mirror_keeps_revocation_and_the_earliest_trial_start() {
    let identity = activation::tests::FakeIdentity.load_or_create().unwrap();
    let mut revoked = Record::new(1000);
    revoked.key = Some(purchase());
    revoked.installation = Some(identity.installation.clone());
    revoked.terminal_status = Some("revoked".into());
    revoked.revision = 5;
    revoked.seal(&identity);
    let legacy = serde_json::to_string(&revoked).unwrap();
    let stored = activation::read_mirror(Err(keyring::Error::NoEntry), || Ok(legacy))
        .unwrap()
        .map(|text| decode_record(&text).unwrap());
    let mut file = Record::new(2000);
    file.installation = Some(identity.installation.clone());
    file.revision = 1;
    file.seal(&identity);
    let merged = Record::reconcile(Some(file), stored, &identity).unwrap();
    assert_eq!(merged.terminal_status.as_deref(), Some("revoked"));
    assert_eq!(merged.first_run, 1000);
    assert!(merged.authenticated(&identity));
}
#[test]
fn rejects_corrupt_trial_records() {
    let mut record = Record::new(1000);
    record.first_run = 2000;
    assert!(decode_record(&serde_json::to_string(&record).unwrap()).is_err());
}
