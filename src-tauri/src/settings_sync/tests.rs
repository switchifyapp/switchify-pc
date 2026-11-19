use super::*;
use crate::state::AppSettings;
use std::collections::VecDeque;

fn untouched() -> Document {
    Local {
        settings: AppSettings::default(),
        profiles: Vec::new(),
        switches: crate::switches::Settings::default(),
        point_scan: crate::point_scan::Config::default(),
        remote: crate::remote_scan::Config::default(),
        panels: Vec::new(),
        menu_overrides: Default::default(),
        panel_extras: Default::default(),
    }
    .document()
}

fn with_dwell(mut document: Document, enabled: bool) -> Document {
    document.app.dwell_click_enabled = enabled;
    document
}

fn with_speed(mut document: Document, speed: usize) -> Document {
    document.point_scan.speed = speed;
    document
}

fn auth(user: &str) -> Authorized {
    Authorized {
        user_id: user.into(),
        access_token: "token".into(),
    }
}

/// (kind, expected revision, payload, updated_by)
type Write = (&'static str, Option<i64>, Value, String);

#[derive(Default)]
struct FakeRemote {
    row: Mutex<Option<Row>>,
    writes: Mutex<Vec<Write>>,
    /// Simulates another computer writing just before our next write.
    interleave: Mutex<VecDeque<Value>>,
    fail: Mutex<bool>,
    fail_update: Mutex<bool>,
}

impl FakeRemote {
    fn holding(document: &Document, revision: i64) -> Self {
        let remote = Self::default();
        *remote.row.lock().unwrap() = Some(Row {
            revision,
            schema_version: SCHEMA_VERSION,
            payload: serde_json::to_value(document).unwrap(),
        });
        remote
    }
    fn document(&self) -> Option<Document> {
        self.row
            .lock()
            .unwrap()
            .as_ref()
            .map(|row| serde_json::from_value(row.payload.clone()).unwrap())
    }
    fn revision(&self) -> Option<i64> {
        self.row.lock().unwrap().as_ref().map(|row| row.revision)
    }
    fn interleave(&self) {
        if let Some(payload) = self.interleave.lock().unwrap().pop_front() {
            let mut row = self.row.lock().unwrap();
            let revision = row.as_ref().map_or(1, |row| row.revision + 1);
            *row = Some(Row {
                revision,
                schema_version: SCHEMA_VERSION,
                payload,
            });
        }
    }
}

impl Remote for FakeRemote {
    fn fetch<'a>(&'a self, _auth: &'a Authorized) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            if *self.fail.lock().unwrap() {
                return Err("offline".into());
            }
            Ok(self.row.lock().unwrap().clone())
        })
    }
    fn create<'a>(
        &'a self,
        _auth: &'a Authorized,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            self.interleave();
            self.writes
                .lock()
                .unwrap()
                .push(("create", None, payload.clone(), updated_by.into()));
            let mut row = self.row.lock().unwrap();
            if row.is_some() {
                return Ok(None);
            }
            *row = Some(Row {
                revision: 1,
                schema_version: SCHEMA_VERSION,
                payload,
            });
            Ok(row.clone())
        })
    }
    fn update<'a>(
        &'a self,
        _auth: &'a Authorized,
        revision: i64,
        payload: Value,
        updated_by: &'a str,
    ) -> BoxFuture<'a, Result<Option<Row>, String>> {
        Box::pin(async move {
            if *self.fail_update.lock().unwrap() {
                return Err("offline".into());
            }
            self.interleave();
            self.writes.lock().unwrap().push((
                "update",
                Some(revision),
                payload.clone(),
                updated_by.into(),
            ));
            let mut row = self.row.lock().unwrap();
            match row.as_mut() {
                Some(current) if current.revision == revision => {
                    current.revision += 1;
                    current.payload = payload;
                    Ok(Some(current.clone()))
                }
                _ => Ok(None),
            }
        })
    }
}

type Normalize = fn(Document) -> Document;

struct FakeHost {
    user: Mutex<Option<String>>,
    local: Mutex<Document>,
    state: Mutex<State>,
    applied: Mutex<Vec<Document>>,
    published: Mutex<Vec<SyncView>>,
    /// Mimics the next apply normalizing what it stores (e.g. profile
    /// versions). Used once.
    transform: Mutex<Option<Normalize>>,
    /// Edits the user makes while a sync is in flight, landing just
    /// before the next apply.
    edits: Mutex<VecDeque<Document>>,
    sync_switches: Mutex<bool>,
    /// Turns switch sync off just before the next apply.
    switch_sync_off_on_apply: Mutex<bool>,
}

impl FakeHost {
    fn new(local: Document) -> Self {
        Self {
            user: Mutex::new(Some("user-1".into())),
            local: Mutex::new(local),
            state: Mutex::default(),
            applied: Mutex::default(),
            published: Mutex::default(),
            transform: Mutex::default(),
            edits: Mutex::default(),
            sync_switches: Mutex::new(true),
            switch_sync_off_on_apply: Mutex::default(),
        }
    }
    fn local_now(&self) -> Document {
        self.local.lock().unwrap().clone()
    }
    fn set_local(&self, document: Document) {
        *self.local.lock().unwrap() = document;
    }
    fn base(&self) -> Option<Base> {
        self.state.lock().unwrap().base.clone()
    }
}

impl Host for FakeHost {
    fn authorize(&self) -> BoxFuture<'_, Result<Option<Authorized>, String>> {
        Box::pin(async move { Ok(self.user.lock().unwrap().as_deref().map(auth)) })
    }
    fn local(&self) -> BoxFuture<'_, Result<Document, String>> {
        Box::pin(async move { Ok(self.local_now()) })
    }
    fn apply(
        &self,
        expected: Document,
        document: Document,
    ) -> BoxFuture<'_, Result<Option<Document>, String>> {
        Box::pin(async move {
            if let Some(edit) = self.edits.lock().unwrap().pop_front() {
                self.set_local(edit);
            }
            if std::mem::take(&mut *self.switch_sync_off_on_apply.lock().unwrap()) {
                *self.sync_switches.lock().unwrap() = false;
            }
            if self.local_now() != expected {
                return Ok(None);
            }
            let document = to_apply(document, &expected, self.sync_switches());
            self.applied.lock().unwrap().push(document.clone());
            // Profiles are stored the way the app stores them (version
            // rules included), which is what can make computers drift.
            let mut document = document;
            if let Some(profiles) =
                portable_settings::merge_profiles(&expected.profiles, document.profiles.clone())
                    .unwrap()
            {
                document.profiles = profiles;
            } else {
                document.profiles = expected.profiles.clone();
            }
            if let Some(panels) =
                crate::custom_panels::plan_replacement(&expected.panels, document.panels.clone())
                    .unwrap()
            {
                document.panels = panels;
            } else {
                document.panels = expected.panels.clone();
            }
            if let Some(overrides) = crate::custom_panels::plan_override_replacement(
                &expected.menu_overrides,
                document.menu_overrides.clone(),
            )
            .unwrap()
            {
                document.menu_overrides = overrides;
            } else {
                document.menu_overrides = expected.menu_overrides.clone();
            }
            if let Some(extras) = crate::panel_extras::plan_replacement(
                &expected.panel_extras,
                document.panel_extras.clone(),
            )
            .unwrap()
            {
                document.panel_extras = extras;
            } else {
                document.panel_extras = expected.panel_extras.clone();
            }
            let transform = self.transform.lock().unwrap().take();
            let stored = transform.map_or(document.clone(), |t| t(document));
            self.set_local(stored.clone());
            Ok(Some(stored))
        })
    }
    fn untouched(&self) -> Document {
        untouched()
    }
    fn sync_switches(&self) -> bool {
        *self.sync_switches.lock().unwrap()
    }
    fn load_state(&self) -> State {
        self.state.lock().unwrap().clone()
    }
    fn save_state(&self, state: &State) -> Result<(), String> {
        *self.state.lock().unwrap() = state.clone();
        Ok(())
    }
    fn publish(&self, view: &SyncView) {
        self.published.lock().unwrap().push(view.clone());
    }
}

fn engine(remote: &Arc<FakeRemote>, host: &Arc<FakeHost>) -> Engine {
    Engine::new(remote.clone(), host.clone())
}

/// Switches of a different interface: other timing and a remote slot.
fn with_switches(mut document: Document, hold_interval_ms: u64) -> Document {
    document.switches.hold_interval_ms = hold_interval_ms;
    document.remote_switches = vec![crate::remote_scan::Slot {
        press_action: None,
        hold_actions: vec![],
        name: Some(format!("Switch {hold_interval_ms}")),
    }];
    document
}

fn keeping_switches(local: Document) -> Arc<FakeHost> {
    let host = Arc::new(FakeHost::new(local));
    *host.sync_switches.lock().unwrap() = false;
    host
}

#[tokio::test]
async fn kept_switches_are_not_uploaded_by_the_first_computer() {
    let remote = Arc::new(FakeRemote::default());
    let host = keeping_switches(with_dwell(with_switches(untouched(), 2000), true));
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::UpToDate);
    assert_eq!(remote.document(), Some(with_dwell(untouched(), true)));
}

#[tokio::test]
async fn kept_switches_survive_the_account_settings() {
    let cloud = with_speed(with_switches(untouched(), 3000), 4);
    let remote = Arc::new(FakeRemote::holding(&cloud, 4));
    let local = with_switches(untouched(), 2000);
    let host = keeping_switches(local.clone());
    let engine = engine(&remote, &host);
    // Only the switches differ, so this computer counts as untouched.
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    assert_eq!(host.local_now(), with_speed(local.clone(), 4));
    assert_eq!(remote.document(), Some(cloud.clone()));
    assert!(remote.writes.lock().unwrap().is_empty());

    // Changing switches here is not a change to sync.
    host.set_local(with_speed(with_switches(untouched(), 2500), 4));
    assert!(!engine.changed_since_sync().await);
    engine.sync(None).await;
    assert!(remote.writes.lock().unwrap().is_empty());

    // Other changes upload with the account's switches.
    host.set_local(with_dwell(
        with_speed(with_switches(untouched(), 2500), 4),
        true,
    ));
    assert!(engine.changed_since_sync().await);
    engine.sync(None).await;
    assert_eq!(remote.document(), Some(with_dwell(cloud.clone(), true)));

    // Another computer's change arrives, leaving these switches alone.
    let elsewhere = with_speed(with_dwell(with_switches(untouched(), 4000), true), 1);
    *remote.row.lock().unwrap() = Some(Row {
        revision: 9,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(&elsewhere).unwrap(),
    });
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    assert_eq!(
        host.local_now(),
        with_speed(with_dwell(with_switches(untouched(), 2500), true), 1)
    );
    assert_eq!(remote.document(), Some(elsewhere));
    assert_eq!(remote.revision(), Some(9));
}

#[tokio::test]
async fn choosing_the_account_settings_keeps_this_computers_switches() {
    let cloud = with_speed(with_switches(untouched(), 3000), 4);
    let remote = Arc::new(FakeRemote::holding(&cloud, 4));
    let local = with_dwell(with_switches(untouched(), 2000), true);
    let host = keeping_switches(local);
    let engine = engine(&remote, &host);
    assert_eq!(engine.sync(None).await.status, Status::NeedsChoice);
    engine.sync(Some(Choice::Cloud)).await;
    assert_eq!(
        host.local_now(),
        with_speed(with_switches(untouched(), 2000), 4)
    );
    assert_eq!(remote.document(), Some(cloud));
}

#[tokio::test]
async fn turning_switch_sync_back_on_shares_this_computers_switches() {
    let cloud = with_switches(untouched(), 3000);
    let remote = Arc::new(FakeRemote::holding(&cloud, 4));
    let local = with_switches(untouched(), 2000);
    let host = keeping_switches(local.clone());
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    assert_eq!(remote.document(), Some(cloud));
    *host.sync_switches.lock().unwrap() = true;
    assert!(engine.changed_since_sync().await);
    engine.sync(None).await;
    assert_eq!(remote.document(), Some(local.clone()));
    assert_eq!(host.local_now(), local);
}

#[tokio::test]
async fn turning_switch_sync_off_during_a_sync_keeps_and_never_uploads_switches() {
    // Both sides changed since the last sync, so the merge applies the
    // account's settings here and uploads the result.
    let cloud = with_speed(with_switches(untouched(), 3000), 4);
    let remote = Arc::new(FakeRemote::holding(&cloud, 5));
    let host = Arc::new(FakeHost::new(with_dwell(
        with_switches(untouched(), 2000),
        true,
    )));
    host.state.lock().unwrap().base = Some(Base {
        revision: 4,
        ..base_of(&untouched())
    });
    *host.switch_sync_off_on_apply.lock().unwrap() = true;
    let engine = engine(&remote, &host);
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    assert_eq!(
        host.local_now(),
        with_speed(with_dwell(with_switches(untouched(), 2000), true), 4)
    );
    assert_eq!(remote.document(), Some(with_dwell(cloud, true)));
}

#[tokio::test]
async fn turning_switch_sync_off_is_noticed_before_an_upload() {
    let remote = Arc::new(FakeRemote::holding(&untouched(), 1));
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    assert!(!engine.turned_off(true));
    *host.sync_switches.lock().unwrap() = false;
    assert!(engine.turned_off(true));
    assert!(!engine.turned_off(false));
}

#[tokio::test]
async fn signed_out_is_off_and_touches_nothing() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    *host.user.lock().unwrap() = None;
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::Off);
    assert!(remote.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn first_computer_creates_the_row() {
    let remote = Arc::new(FakeRemote::default());
    let local = with_dwell(untouched(), true);
    let host = Arc::new(FakeHost::new(local.clone()));
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::UpToDate);
    assert!(view.last_synced_at.is_some());
    assert_eq!(remote.document(), Some(local.clone()));
    let base = host.base().unwrap();
    assert_eq!((base.user_id.as_str(), base.revision), ("user-1", 1));
    let writes = remote.writes.lock().unwrap();
    let install_id = &writes[0].3;
    assert!(
        uuid::Uuid::parse_str(install_id).is_ok(),
        "random install id"
    );
}

#[tokio::test]
async fn untouched_computer_takes_the_account_settings() {
    let cloud = with_dwell(untouched(), true);
    let remote = Arc::new(FakeRemote::holding(&cloud, 4));
    let host = Arc::new(FakeHost::new(untouched()));
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::UpToDate);
    assert_eq!(host.local_now(), cloud);
    assert!(remote.writes.lock().unwrap().is_empty());
    assert_eq!(host.base().unwrap().revision, 4);
}

#[tokio::test]
async fn customised_computer_asks_before_replacing_anything() {
    let cloud = with_dwell(untouched(), true);
    let local = with_speed(untouched(), 4);
    let remote = Arc::new(FakeRemote::holding(&cloud, 2));
    let host = Arc::new(FakeHost::new(local.clone()));
    let engine = engine(&remote, &host);
    assert_eq!(engine.sync(None).await.status, Status::NeedsChoice);
    assert_eq!(host.local_now(), local);
    assert!(remote.writes.lock().unwrap().is_empty());
    assert!(host.base().is_none());

    // Keep this computer's settings: uploaded over the account's copy.
    assert_eq!(
        engine.sync(Some(Choice::Local)).await.status,
        Status::UpToDate
    );
    assert_eq!(remote.document(), Some(local));
    assert_eq!(remote.revision(), Some(3));
}

#[tokio::test]
async fn choosing_cloud_replaces_this_computers_settings() {
    let cloud = with_dwell(untouched(), true);
    let remote = Arc::new(FakeRemote::holding(&cloud, 2));
    let host = Arc::new(FakeHost::new(with_speed(untouched(), 4)));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    assert_eq!(
        engine.sync(Some(Choice::Cloud)).await.status,
        Status::UpToDate
    );
    assert_eq!(host.local_now(), cloud);
    assert!(remote.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn identical_settings_need_no_choice() {
    let local = with_speed(untouched(), 4);
    let remote = Arc::new(FakeRemote::holding(&local, 2));
    let host = Arc::new(FakeHost::new(local));
    assert_eq!(
        engine(&remote, &host).sync(None).await.status,
        Status::UpToDate
    );
    assert!(remote.writes.lock().unwrap().is_empty());
    assert_eq!(host.base().unwrap().revision, 2);
}

#[tokio::test]
async fn local_change_uploads_on_the_last_seen_revision() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    host.set_local(with_dwell(untouched(), true));
    assert!(engine.changed_since_sync().await);
    engine.sync(None).await;
    let last = remote.writes.lock().unwrap().last().cloned().unwrap();
    assert_eq!((last.0, last.1), ("update", Some(1)));
    assert_eq!(remote.revision(), Some(2));
    assert!(!engine.changed_since_sync().await);
}

#[tokio::test]
async fn remote_change_is_applied_and_not_echoed() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    // Another computer changes the account's copy.
    let other = with_dwell(untouched(), true);
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(&other).unwrap(),
    });
    engine.sync(None).await;
    assert_eq!(host.local_now(), other);
    assert_eq!(remote.writes.lock().unwrap().len(), 1, "only the create");
    assert_eq!(host.base().unwrap().revision, 2);
}

#[tokio::test]
async fn changes_on_both_sides_merge_by_section() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(with_dwell(untouched(), true)).unwrap(),
    });
    host.set_local(with_speed(untouched(), 4));
    engine.sync(None).await;
    let merged = with_speed(with_dwell(untouched(), true), 4);
    assert_eq!(host.local_now(), merged);
    assert_eq!(remote.document(), Some(merged));
    assert_eq!(remote.revision(), Some(3));
}

fn base_of(document: &Document) -> Base {
    Base {
        user_id: "user-1".into(),
        revision: 1,
        cloud: document.clone(),
        local: document.clone(),
    }
}

#[test]
fn the_account_wins_a_section_changed_on_both_sides() {
    let base = untouched();
    let local = with_speed(base.clone(), 4);
    let cloud = with_speed(base.clone(), 1);
    assert_eq!(merge(&base_of(&base), &local, &cloud).point_scan.speed, 1);
}

fn profile(id: &str, name: &str) -> SwitchProfile {
    SwitchProfile {
        id: id.into(),
        version: 1,
        name: name.into(),
        provider: "mapped".into(),
        built_in: false,
        bindings: (1..=8)
            .map(|switch_id| crate::state::SwitchBinding {
                switch_id,
                binding_type: "none".into(),
                value: None,
                keys: None,
                click_count: None,
            })
            .collect(),
    }
}
const A: &str = "6f1c1a52-6a0e-4c2b-9d0f-2f8a7b9e4c11";
const B: &str = "0b7e2d4a-1c3f-4e5a-8b9c-7d6e5f4a3b22";
const C: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

fn with_profiles(mut document: Document, profiles: Vec<SwitchProfile>) -> Document {
    document.profiles = profiles;
    document
}

fn names(document: &Document) -> Vec<String> {
    document.profiles.iter().map(|p| p.name.clone()).collect()
}

#[test]
fn profiles_added_on_two_computers_are_both_kept() {
    let base = with_profiles(untouched(), vec![profile(A, "Desk")]);
    let local = with_profiles(untouched(), vec![profile(A, "Desk"), profile(B, "Laptop")]);
    let cloud = with_profiles(untouched(), vec![profile(A, "Desk"), profile(C, "Tablet")]);
    let merged = merge(&base_of(&base), &local, &cloud);
    assert_eq!(names(&merged), ["Desk", "Tablet", "Laptop"]);
}

#[test]
fn profile_deletions_merge_with_edits() {
    let base = with_profiles(untouched(), vec![profile(A, "Desk"), profile(B, "Laptop")]);
    // The account deleted both. This computer renamed A and left B alone.
    let local = with_profiles(
        untouched(),
        vec![profile(A, "Desk 2"), profile(B, "Laptop")],
    );
    let cloud = with_profiles(untouched(), vec![]);
    let merged = merge(&base_of(&base), &local, &cloud);
    assert_eq!(names(&merged), ["Desk 2"], "an edit survives a deletion");
    // Deleted here and untouched in the account: stays deleted.
    let local = with_profiles(untouched(), vec![profile(B, "Laptop")]);
    let merged = merge(&base_of(&base), &local, &base);
    assert_eq!(names(&merged), ["Laptop"]);
}

#[test]
fn merged_profile_names_stay_unique() {
    let base = untouched();
    let local = with_profiles(untouched(), vec![profile(B, "Desk")]);
    let cloud = with_profiles(untouched(), vec![profile(A, "desk")]);
    let merged = merge(&base_of(&base), &local, &cloud);
    assert_eq!(names(&merged), ["desk", "Desk (2)"]);
}

#[test]
fn a_version_only_difference_is_not_an_edit() {
    // The review case: the account's copy differs from the base only in
    // version; the local copy was renamed. The rename must win.
    let mut base_p = profile(A, "Desk");
    base_p.version = 3;
    let mut local_p = profile(A, "Desk renamed");
    local_p.version = 4;
    let mut cloud_p = profile(A, "Desk");
    cloud_p.version = 2;
    let base = with_profiles(untouched(), vec![base_p]);
    let merged = merge(
        &base_of(&base),
        &with_profiles(untouched(), vec![local_p]),
        &with_profiles(untouched(), vec![cloud_p]),
    );
    assert_eq!(names(&merged), ["Desk renamed"]);
    assert_eq!(merged.profiles[0].version, 4, "versions never go backwards");
}

#[test]
fn a_version_only_difference_does_not_resurrect_a_deletion() {
    let mut base_p = profile(A, "Desk");
    base_p.version = 3;
    let mut local_p = base_p.clone();
    local_p.version = 5;
    let base = with_profiles(untouched(), vec![base_p]);
    let merged = merge(
        &base_of(&base),
        &with_profiles(untouched(), vec![local_p]),
        &with_profiles(untouched(), vec![]),
    );
    assert!(merged.profiles.is_empty());
}

#[tokio::test]
async fn two_computers_settle_after_losing_profile_races() {
    // The review sequence: each computer loses one same-profile race,
    // which bumps its version for that profile. Afterwards any change
    // must settle instead of the two uploading to each other forever.
    let start = with_profiles(untouched(), vec![profile(A, "P"), profile(B, "Q")]);
    let remote = Arc::new(FakeRemote::default());
    let host_a = Arc::new(FakeHost::new(start.clone()));
    let host_b = Arc::new(FakeHost::new(start.clone()));
    let (a, b) = (engine(&remote, &host_a), engine(&remote, &host_b));
    a.sync(None).await;
    b.sync(None).await;
    let rename = |host: &FakeHost, index: usize, name: &str| {
        let mut document = host.local_now();
        document.profiles[index].name = name.into();
        document.profiles[index].version += 1;
        host.set_local(document);
    };
    // Both edit P; A uploads first and B loses.
    rename(&host_a, 0, "P from A");
    rename(&host_b, 0, "P from B");
    a.sync(None).await;
    b.sync(None).await;
    a.sync(None).await;
    // Both edit Q; B uploads first and A loses.
    rename(&host_b, 1, "Q from B");
    rename(&host_a, 1, "Q from A");
    b.sync(None).await;
    a.sync(None).await;
    b.sync(None).await;
    // One unrelated change, then many periodic checks on both.
    host_a.set_local(with_dwell(host_a.local_now(), true));
    let writes_before = remote.writes.lock().unwrap().len();
    for _ in 0..10 {
        a.sync(None).await;
        b.sync(None).await;
    }
    let writes = remote.writes.lock().unwrap().len() - writes_before;
    assert!(writes <= 2, "settled after {writes} uploads, not a loop");
    assert_eq!(host_a.local_now(), host_b.local_now());
    assert_eq!(remote.document(), Some(host_a.local_now()));
    assert_eq!(names(&host_a.local_now()), ["P from A", "Q from B"]);
}

fn with_panels(mut document: Document, panels: Vec<crate::custom_panels::CustomPanel>) -> Document {
    document.panels = panels;
    document
}

#[test]
fn panels_added_on_two_computers_are_both_kept() {
    use crate::custom_panels::sample;
    let base = with_panels(untouched(), vec![sample("a", "Desk")]);
    let local = with_panels(
        untouched(),
        vec![sample("a", "Desk"), sample("b", "Laptop")],
    );
    let cloud = with_panels(
        untouched(),
        vec![sample("a", "Desk"), sample("c", "Tablet")],
    );
    let merged = merge(&base_of(&base), &local, &cloud);
    let names: Vec<_> = merged.panels.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Desk", "Tablet", "Laptop"]);
}

#[tokio::test]
async fn too_many_panels_after_a_merge_is_a_clear_error() {
    use crate::custom_panels::sample;
    let make = |prefix: u32| -> Vec<crate::custom_panels::CustomPanel> {
        (0..20)
            .map(|n| sample(&format!("p{prefix}-{n}"), &format!("Panel {prefix}-{n}")))
            .collect()
    };
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(with_panels(untouched(), make(1))).unwrap(),
    });
    host.set_local(with_panels(untouched(), make(2)));
    let view = engine.sync(None).await;
    assert_eq!(view.status, Status::Error);
    assert_eq!(view.message.as_deref(), Some(TOO_MANY_PANELS));
    assert!(host.applied.lock().unwrap().is_empty(), "nothing applied");
}

#[tokio::test]
async fn too_many_profiles_after_a_merge_is_a_clear_error() {
    let make = |prefix: u32| -> Vec<SwitchProfile> {
        (0..20)
            .map(|n| {
                profile(
                    &format!("{prefix:08x}-0000-4000-8000-{n:012x}"),
                    &format!("P{prefix}-{n}"),
                )
            })
            .collect()
    };
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(with_profiles(untouched(), make(1))).unwrap(),
    });
    host.set_local(with_profiles(untouched(), make(2)));
    let view = engine.sync(None).await;
    assert_eq!(view.status, Status::Error);
    assert_eq!(view.message.as_deref(), Some(TOO_MANY_PROFILES));
    assert!(host.applied.lock().unwrap().is_empty(), "nothing applied");
}

#[tokio::test]
async fn normalization_on_apply_never_hides_a_later_local_edit() {
    // The review case: an apply stores a normalized value, another
    // computer then changes a different section, and the user edits the
    // normalized section here. The edit must survive.
    let mut cloud = with_dwell(untouched(), true);
    let remote = Arc::new(FakeRemote::holding(&cloud, 2));
    let host = Arc::new(FakeHost::new(untouched()));
    *host.transform.lock().unwrap() = Some(|document| with_speed(document, 3));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    assert_eq!(host.local_now().point_scan.speed, 3);
    // Another computer changes only the cursor.
    cloud.app.cursor_crosshairs = true;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 3,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(&cloud).unwrap(),
    });
    // The user changes scan speed here.
    host.set_local(with_speed(host.local_now(), 4));
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    let local = host.local_now();
    assert_eq!(local.point_scan.speed, 4, "local edit kept");
    assert!(
        local.app.cursor_crosshairs,
        "other computer's change applied"
    );
    assert_eq!(remote.document().unwrap().point_scan.speed, 4);
}

#[tokio::test]
async fn an_edit_made_during_a_sync_is_not_reverted() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(with_dwell(untouched(), true)).unwrap(),
    });
    // While the account is being read, the user changes scan speed.
    host.edits
        .lock()
        .unwrap()
        .push_back(with_speed(untouched(), 4));
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    let merged = with_speed(with_dwell(untouched(), true), 4);
    assert_eq!(host.local_now(), merged);
    assert_eq!(remote.document(), Some(merged));
}

#[tokio::test]
async fn a_failed_upload_after_apply_keeps_local_changes_pending() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    *remote.row.lock().unwrap() = Some(Row {
        revision: 2,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(with_dwell(untouched(), true)).unwrap(),
    });
    host.set_local(with_speed(untouched(), 4));
    *remote.fail_update.lock().unwrap() = true;
    assert_eq!(engine.sync(None).await.status, Status::Error);
    assert_eq!(
        host.local_now(),
        with_speed(with_dwell(untouched(), true), 4)
    );
    assert!(
        engine.changed_since_sync().await,
        "speed change still to upload"
    );
    *remote.fail_update.lock().unwrap() = false;
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    let uploaded = remote.document().unwrap();
    assert_eq!(uploaded.point_scan.speed, 4);
    assert!(uploaded.app.dwell_click_enabled);
}

#[tokio::test]
async fn losing_a_write_race_re_reads_and_merges() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    host.set_local(with_speed(untouched(), 4));
    // Another computer writes between our read and our write.
    remote
        .interleave
        .lock()
        .unwrap()
        .push_back(serde_json::to_value(with_dwell(untouched(), true)).unwrap());
    assert_eq!(engine.sync(None).await.status, Status::UpToDate);
    let merged = with_speed(with_dwell(untouched(), true), 4);
    assert_eq!(remote.document(), Some(merged.clone()));
    assert_eq!(host.local_now(), merged);
}

#[tokio::test]
async fn create_race_falls_back_to_the_other_computers_row() {
    let remote = Arc::new(FakeRemote::default());
    let other = with_dwell(untouched(), true);
    remote
        .interleave
        .lock()
        .unwrap()
        .push_back(serde_json::to_value(&other).unwrap());
    let host = Arc::new(FakeHost::new(untouched()));
    assert_eq!(
        engine(&remote, &host).sync(None).await.status,
        Status::UpToDate
    );
    assert_eq!(
        host.local_now(),
        other,
        "untouched install takes the winner's"
    );
}

#[tokio::test]
async fn newer_app_settings_are_never_overwritten() {
    let remote = Arc::new(FakeRemote::default());
    *remote.row.lock().unwrap() = Some(Row {
        revision: 9,
        schema_version: SCHEMA_VERSION + 1,
        payload: json!({"futureShape": true}),
    });
    let host = Arc::new(FakeHost::new(with_speed(untouched(), 4)));
    let engine = engine(&remote, &host);
    let view = engine.sync(Some(Choice::Local)).await;
    assert_eq!(view.status, Status::UpdateRequired);
    assert!(view.message.unwrap().contains("Update Switchify PC"));
    assert!(remote.writes.lock().unwrap().is_empty());
    assert!(host.applied.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invalid_account_copy_is_an_error_and_not_overwritten() {
    let remote = Arc::new(FakeRemote::default());
    *remote.row.lock().unwrap() = Some(Row {
        revision: 3,
        schema_version: SCHEMA_VERSION,
        payload: json!({"schemaVersion": 1}),
    });
    let host = Arc::new(FakeHost::new(untouched()));
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::Error);
    assert!(remote.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn another_accounts_base_is_never_merged() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    // A different account (e.g. deleted and recreated) with its own row.
    *host.user.lock().unwrap() = Some("user-2".into());
    let cloud = with_dwell(untouched(), true);
    *remote.row.lock().unwrap() = Some(Row {
        revision: 1,
        schema_version: SCHEMA_VERSION,
        payload: serde_json::to_value(&cloud).unwrap(),
    });
    host.set_local(with_speed(untouched(), 4));
    // Same revision number as the old base, but it must not count.
    assert_eq!(engine.sync(None).await.status, Status::NeedsChoice);
}

#[test]
fn a_base_saved_before_panels_still_loads_from_disk() {
    let base = Base {
        user_id: "u".into(),
        revision: 3,
        cloud: untouched(),
        local: untouched(),
    };
    let mut value = serde_json::to_value(&base).unwrap();
    for side in ["cloud", "local"] {
        let document = value[side].as_object_mut().unwrap();
        document.remove("panels");
        document["schemaVersion"] = serde_json::json!(6);
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    let loaded: Base = serde_json::from_slice(&bytes).unwrap();
    assert!(loaded.cloud.panels.is_empty() && loaded.local.panels.is_empty());
    assert_eq!(loaded.cloud.schema_version, 6);
}

#[tokio::test]
async fn updating_from_an_older_version_uploads_nothing_until_a_change() {
    for version in 1..SCHEMA_VERSION {
        let local = with_dwell(untouched(), true);
        let remote = Arc::new(FakeRemote::default());
        let host = Arc::new(FakeHost::new(local));
        engine(&remote, &host).sync(None).await;
        // What the older build saved and uploaded.
        {
            let mut state = host.state.lock().unwrap();
            let base = state.base.as_mut().unwrap();
            base.cloud.schema_version = version;
            base.local.schema_version = version;
        }
        {
            let mut row = remote.row.lock().unwrap();
            let row = row.as_mut().unwrap();
            row.schema_version = version;
            row.payload = portable_settings::written_by(row.payload.clone(), version);
        }
        let writes = remote.writes.lock().unwrap().len();
        let updated = engine(&remote, &host);
        assert!(!updated.changed_since_sync().await);
        assert_eq!(updated.sync(None).await.status, Status::UpToDate);
        assert_eq!(remote.writes.lock().unwrap().len(), writes);
    }
}

#[tokio::test]
async fn normalized_apply_becomes_the_base_so_nothing_ping_pongs() {
    let cloud = with_dwell(untouched(), true);
    let remote = Arc::new(FakeRemote::holding(&cloud, 2));
    let host = Arc::new(FakeHost::new(untouched()));
    // Applying stores something slightly different (like a profile
    // version bump); that stored copy must become the local base.
    *host.transform.lock().unwrap() = Some(|document| with_speed(document, 3));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    assert_eq!(host.base().unwrap().local, host.local_now());
    assert!(!engine.changed_since_sync().await);
    engine.sync(None).await;
    assert!(remote.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn network_failure_reports_an_error() {
    let remote = Arc::new(FakeRemote::default());
    *remote.fail.lock().unwrap() = true;
    let host = Arc::new(FakeHost::new(untouched()));
    let view = engine(&remote, &host).sync(None).await;
    assert_eq!(view.status, Status::Error);
    assert_eq!(view.message.as_deref(), Some("offline"));
}

#[tokio::test]
async fn forget_clears_the_base_and_turns_off() {
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(untouched()));
    let engine = engine(&remote, &host);
    engine.sync(None).await;
    engine.forget().await;
    assert!(host.base().is_none());
    assert_eq!(engine.view().status, Status::Off);
    assert!(
        !host.state.lock().unwrap().install_id.is_empty(),
        "install id kept"
    );
}

#[tokio::test]
async fn oversized_documents_are_not_uploaded() {
    let mut local = untouched();
    local.remote_switches[0].name = Some("x".repeat(MAX_PAYLOAD_BYTES));
    assert!(payload(&local).is_err());
    let remote = Arc::new(FakeRemote::default());
    let host = Arc::new(FakeHost::new(local));
    assert_eq!(
        engine(&remote, &host).sync(None).await.status,
        Status::Error
    );
    assert!(remote.writes.lock().unwrap().is_empty());
}

/// Against a local `supabase start` stack (switchify-supabase). Run with:
/// SWITCHIFY_LOCAL_SUPABASE_URL=http://127.0.0.1:54321
/// SWITCHIFY_LOCAL_SUPABASE_KEY=<local publishable key>
/// cargo test settings_sync::tests::local_stack -- --ignored
#[tokio::test]
#[ignore = "needs a local Supabase stack"]
async fn local_stack_round_trips_the_document_and_enforces_revisions() {
    let url = std::env::var("SWITCHIFY_LOCAL_SUPABASE_URL").unwrap();
    let key = std::env::var("SWITCHIFY_LOCAL_SUPABASE_KEY").unwrap();
    let client = reqwest::Client::new();
    let mut users = Vec::new();
    for _ in 0..2 {
        let signup: Value = client
            .post(format!("{url}/auth/v1/signup"))
            .header("apikey", &key)
            .json(&json!({
                "email": format!("sync-{}@example.test", uuid::Uuid::new_v4()),
                "password": "local-only-password",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        users.push(Authorized {
            user_id: signup["user"]["id"].as_str().unwrap().into(),
            access_token: signup["access_token"].as_str().unwrap().into(),
        });
    }
    let (me, other) = (&users[0], &users[1]);
    let remote = HttpRemote::new(url, key);

    assert_eq!(remote.fetch(me).await.unwrap(), None);
    // The untouched document has explicit nulls (e.g. unassigned remote
    // slots); jsonb must keep them for the strict parser.
    let document = untouched();
    let value = payload(&document).unwrap();
    assert!(value.to_string().contains("null"));
    let created = remote
        .create(me, value.clone(), "install")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(created.revision, 1);
    let fetched = remote.fetch(me).await.unwrap().unwrap();
    assert_eq!(
        portable_settings::parse(fetched.payload, fetched.schema_version).unwrap(),
        document
    );
    assert_eq!(
        remote.create(me, value.clone(), "install").await.unwrap(),
        None,
        "409"
    );

    let changed = payload(&with_dwell(untouched(), true)).unwrap();
    let updated = remote
        .update(me, 1, changed.clone(), "install")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert_eq!(
        remote.update(me, 1, value, "install").await.unwrap(),
        None,
        "stale"
    );

    // Another account sees nothing of mine and cannot write to it.
    assert_eq!(remote.fetch(other).await.unwrap(), None);
    let forged = Authorized {
        user_id: me.user_id.clone(),
        access_token: other.access_token.clone(),
    };
    assert_eq!(
        remote.update(&forged, 2, json!({}), "x").await.unwrap(),
        None
    );
    assert_eq!(remote.fetch(me).await.unwrap().unwrap().payload, changed);
}
