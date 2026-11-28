use super::*;
use crate::scanning::Action;
use crate::state::{built_in_profiles, SwitchBinding};
use serde_json::json;

fn binding(id: &str, key: &str, action: Action) -> switches::Binding {
    switches::Binding {
        id: id.into(),
        name: action.label().into(),
        key: key.into(),
        press_action: action,
        hold_actions: vec![],
    }
}

fn custom_profile(id: &str, name: &str, first_key: &str) -> SwitchProfile {
    SwitchProfile {
        id: id.into(),
        version: 1,
        name: name.into(),
        provider: "mapped".into(),
        built_in: false,
        bindings: (1..=8)
            .map(|switch_id| SwitchBinding {
                switch_id,
                binding_type: if switch_id == 1 { "key" } else { "none" }.into(),
                value: (switch_id == 1).then(|| first_key.into()),
                keys: None,
                click_count: None,
            })
            .collect(),
    }
}

const PROFILE_A: &str = "6f1c1a52-6a0e-4c2b-9d0f-2f8a7b9e4c11";
const PROFILE_B: &str = "0b7e2d4a-1c3f-4e5a-8b9c-7d6e5f4a3b22";

fn local() -> Local {
    let mut profiles = built_in_profiles(false);
    profiles.push(custom_profile(PROFILE_A, "Desk", "Space"));
    Local {
        settings: AppSettings {
            start_with_system: true,
            share_diagnostics: true,
            auto_approve_account_devices: false,
            text_size_percent: 150,
            ..AppSettings::default()
        },
        profiles,
        switches: switches::Settings {
            bindings: vec![
                binding("a", "Space", Action::Select),
                binding("b", "Enter", Action::Next),
            ],
            ..switches::Settings::default()
        },
        point_scan: point_scan::Config {
            control_mode: point_scan::ControlMode::Mouse,
            select_key: "F1".into(),
            ..point_scan::Config::default()
        },
        remote: remote_scan::Config {
            revision: 7,
            ..remote_scan::Config::default()
        },
        panels: vec![custom_panels::sample("greetings", "Greetings")],
        menu_overrides: BTreeMap::new(),
        panel_extras: BTreeMap::new(),
    }
}

#[test]
fn version_eight_documents_read_with_no_extra_rows() {
    let hello = custom_panels::Cell {
        label: "Hello".into(),
        action: crate::panel_actions::PanelAction::Text {
            text: "Hello".into(),
        },
        span: 1,
        color: None,
    };
    let mut local = local();
    local.panel_extras.insert(
        "keyboard".into(),
        crate::panel_extras::PanelExtras::rows(vec![vec![hello.clone()]]),
    );
    let mut cloud = local.clone();
    cloud.panel_extras.clear();
    let version_eight = written_by(serde_json::to_value(cloud.document()).unwrap(), 8);
    assert!(version_eight.get("panelExtras").is_none());
    assert_eq!(version_eight["schemaVersion"], json!(8));
    let parsed = parse(version_eight.clone(), 8).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert!(parsed.panel_extras.is_empty());
    // Applying it removes this computer's extra rows, as a removal elsewhere would.
    assert_eq!(
        plan(&local, parsed).unwrap().panel_extras,
        Some(BTreeMap::new())
    );
    let mut early = version_eight.clone();
    early["panelExtras"] = json!({});
    assert!(matches!(parse(early, 8), Err(ParseError::Invalid(_))));
    let mut missing = version_eight.clone();
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // A version 1 document still climbs every step.
    let oldest = written_by(serde_json::to_value(cloud.document()).unwrap(), 1);
    assert_eq!(parse(oldest, 1).unwrap().schema_version, SCHEMA_VERSION);
    // Extra rows sync, and a panel that takes none refuses the document.
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(
        value["panelExtras"]["keyboard"]["rows"][0][0]["label"],
        json!("Hello")
    );
    let mut bad = local.document();
    bad.panel_extras.insert(
        "home".into(),
        crate::panel_extras::PanelExtras::rows(vec![vec![hello]]),
    );
    assert!(plan(&local, bad).is_err());
}

#[test]
fn version_twenty_documents_read_with_no_tile_colours_and_colours_sync() {
    use crate::panel_theme::TileColor;
    let hello = custom_panels::Cell {
        label: "Hello".into(),
        action: crate::panel_actions::PanelAction::Text {
            text: "Hello".into(),
        },
        span: 1,
        color: None,
    };
    let mut plain = local();
    plain.menu_overrides.insert(
        "home".into(),
        crate::builtin_panels::document(crate::scan_menu::Kind::Home, &Default::default(), false),
    );
    plain.panel_extras.insert(
        "keyboard".into(),
        crate::panel_extras::PanelExtras {
            rows: vec![vec![hello.clone()]],
            columns: vec![vec![hello]],
        },
    );
    // A document without colours is, tile for tile, what version 20 wrote.
    let current = serde_json::to_value(plain.document()).unwrap();
    assert!(current["panels"][0]["rows"][0][0].get("color").is_none());
    let version_twenty = written_by(current, 20);
    let parsed = parse(version_twenty.clone(), 20).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed, plain.document());
    assert!(plan(&plain, parsed).unwrap().is_empty());
    // A colour in each place a tile can be syncs and applies.
    let mut coloured = plain.clone();
    coloured.panels[0].rows[0][0].color = Some(TileColor::Green);
    coloured.menu_overrides.get_mut("home").unwrap().rows[0][0].color = Some(TileColor::Blue);
    let keyboard = coloured.panel_extras.get_mut("keyboard").unwrap();
    keyboard.rows[0][0].color = Some(TileColor::Pink);
    keyboard.columns[0][0].color = Some(TileColor::Grey);
    let value = serde_json::to_value(coloured.document()).unwrap();
    assert_eq!(value["panels"][0]["rows"][0][0]["color"], json!("green"));
    assert_eq!(
        value["menuOverrides"]["home"]["rows"][0][0]["color"],
        json!("blue")
    );
    assert_eq!(
        value["panelExtras"]["keyboard"]["columns"][0][0]["color"],
        json!("grey")
    );
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(parsed, coloured.document());
    let planned = plan(&plain, parsed).unwrap();
    assert_eq!(
        planned.panels.unwrap()[0].rows[0][0].color,
        Some(TileColor::Green)
    );
    assert!(planned.menu_overrides.is_some() && planned.panel_extras.is_some());
    // Version 20 had no colours, so a version 20 document with one, wherever
    // it is, is refused, as is a colour that is not on the list.
    for pointer in [
        "/panels/0/rows/0/0",
        "/menuOverrides/home/rows/0/0",
        "/panelExtras/keyboard/rows/0/0",
        "/panelExtras/keyboard/columns/0/0",
    ] {
        let mut early = version_twenty.clone();
        early.pointer_mut(pointer).unwrap()["color"] = json!("red");
        assert!(
            matches!(parse(early, 20), Err(ParseError::Invalid(_))),
            "{pointer}"
        );
    }
    let mut unknown = value;
    unknown["panels"][0]["rows"][0][0]["color"] = json!("teal");
    assert!(matches!(
        parse(unknown, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
}

#[test]
fn version_nine_documents_keep_their_rows_and_gain_no_columns() {
    let hello = custom_panels::Cell {
        label: "Hello".into(),
        action: crate::panel_actions::PanelAction::Text {
            text: "Hello".into(),
        },
        span: 1,
        color: None,
    };
    let mut local = local();
    local.panel_extras.insert(
        "mouse".into(),
        crate::panel_extras::PanelExtras {
            rows: vec![vec![hello.clone()]],
            columns: vec![vec![hello.clone()]],
        },
    );
    let version_nine = written_by(serde_json::to_value(local.document()).unwrap(), 9);
    assert_eq!(
        version_nine["panelExtras"]["mouse"][0][0]["label"],
        json!("Hello")
    );
    let parsed = parse(version_nine.clone(), 9).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.panel_extras["mouse"].rows, vec![vec![hello]]);
    assert!(parsed.panel_extras["mouse"].columns.is_empty());
    // Applying it drops this computer's columns, as a removal elsewhere would.
    let planned = plan(&local, parsed).unwrap().panel_extras.unwrap();
    assert!(planned["mouse"].columns.is_empty());
    let mut early = version_nine.clone();
    early["panelExtras"]["mouse"] = json!({ "rows": [], "columns": [] });
    assert!(matches!(parse(early, 9), Err(ParseError::Invalid(_))));
}

#[test]
fn version_eleven_assignments_upgrade_and_panel_actions_round_trip() {
    let original = local().document();
    let old = written_by(serde_json::to_value(&original).unwrap(), 11);
    assert_eq!(parse(old, 11).unwrap(), original);
    let mut document = original;
    document.remote_switches[0].press_action = Some(crate::scanning::Action::OpenMenu(
        crate::scan_menu::Kind::Editing,
    ));
    document.remote_switches[0].hold_actions =
        vec![crate::scanning::Action::OpenCustomPanel("greetings".into())];
    let value = serde_json::to_value(&document).unwrap();
    assert_eq!(parse(value, SCHEMA_VERSION).unwrap(), document);
}

#[test]
fn version_twelve_documents_drop_the_old_prediction_options() {
    let original = local().document();
    let current = serde_json::to_value(&original).unwrap();
    for field in [
        "enhancedWordPrediction",
        "nearbyKeyPredictions",
        "abbreviationPredictions",
    ] {
        assert!(current["pointScan"].get(field).is_none());
    }
    // As v1.8.x wrote it: enhancedWordPrediction always, false.
    let old = written_by(current.clone(), 12);
    assert_eq!(old["pointScan"]["enhancedWordPrediction"], json!(false));
    assert_eq!(parse(old.clone(), 12).unwrap(), original);
    // A later 12 build also wrote the opt-in options when they were on.
    let mut opted_in = old.clone();
    opted_in["pointScan"]["enhancedWordPrediction"] = json!(true);
    opted_in["pointScan"]["nearbyKeyPredictions"] = json!(true);
    opted_in["pointScan"]["abbreviationPredictions"] = json!(true);
    assert_eq!(parse(opted_in, 12).unwrap(), original);
    // Shapes version 12 never wrote are refused, not reset.
    let refused: [fn(&mut Value); 4] = [
        |v| {
            v["pointScan"]
                .as_object_mut()
                .unwrap()
                .remove("enhancedWordPrediction");
        },
        |v| v["pointScan"]["enhancedWordPrediction"] = json!("yes"),
        |v| v["pointScan"]["nearbyKeyPredictions"] = json!(false),
        |v| v["pointScan"]["abbreviationPredictions"] = json!(1),
    ];
    for corrupt in refused {
        let mut value = old.clone();
        corrupt(&mut value);
        assert!(
            matches!(parse(value.clone(), 12), Err(ParseError::Invalid(_))),
            "accepted {value}"
        );
    }
    // A current document still carrying an old option is refused.
    let mut stale = current;
    stale["pointScan"]["enhancedWordPrediction"] = json!(false);
    assert!(matches!(
        parse(stale, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // An older build reads a newer document as newer, not invalid.
    assert_eq!(
        parse(serde_json::to_value(&original).unwrap(), SCHEMA_VERSION + 1),
        Err(ParseError::Newer(SCHEMA_VERSION + 1))
    );
}

#[test]
fn version_ten_documents_read_with_every_timing_off() {
    let mut local = local();
    local.switches.accept_ms = 300;
    local.switches.ignore_repeat_ms = 1000;
    local.point_scan.scan_preferences.first_item_ms = 1500;
    local.point_scan.scan_preferences.group_rate_percent = 200;
    let current = serde_json::to_value(local.document()).unwrap();
    assert_eq!(current["switches"]["acceptMs"], json!(300));
    assert_eq!(current["switches"]["ignoreRepeatMs"], json!(1000));
    let parsed = parse(current.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(parsed.switches.accept_ms, 300);
    assert_eq!(parsed.point_scan.scan_preferences.group_rate_percent, 200);
    let version_ten = written_by(current, 10);
    let parsed = parse(version_ten.clone(), 10).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.switches.accept_ms, 0);
    assert_eq!(parsed.switches.ignore_repeat_ms, 0);
    let preferences = &parsed.point_scan.scan_preferences;
    assert_eq!(preferences.first_item_ms, 0);
    assert_eq!(preferences.group_rate_percent, 100);
    // Switch timing that is off is left out, as version 10 wrote it.
    let off = serde_json::to_value(&parsed).unwrap();
    assert!(off["switches"].get("acceptMs").is_none());
    assert_eq!(off["switches"], version_ten["switches"]);
    for (pointer, field) in [
        ("/switches", "acceptMs"),
        ("/pointScan/scanPreferences", "firstItemMs"),
    ] {
        let mut early = version_ten.clone();
        early.pointer_mut(pointer).unwrap()[field] = json!(0);
        assert!(matches!(parse(early, 10), Err(ParseError::Invalid(_))));
    }
    // Timing a build does not allow is refused, not reset.
    let mut lossy = serde_json::to_value(local.document()).unwrap();
    lossy["pointScan"]["scanPreferences"]["groupRatePercent"] = json!(175);
    assert!(parse(lossy, SCHEMA_VERSION).is_err());
}

#[test]
fn document_excludes_device_only_values() {
    let document = local().document();
    let value = serde_json::to_value(&document).unwrap();
    let text = value.to_string();
    for field in [
        "startWithSystem",
        "shareDiagnostics",
        "autoApproveAccountDevices",
        "textSizePercent",
        "learnWordPrediction",
        "telemetry",
        "desktopId",
        "pairedDevices",
        "setupShown",
        "setupCompleted",
        "revision",
    ] {
        assert!(!text.contains(field), "document leaked {field}");
    }
    assert!(document.profiles.iter().all(|profile| !profile.built_in));
    assert_eq!(document.profiles.len(), 1);
    assert_eq!(
        document.point_scan.control_mode,
        point_scan::Config::default().control_mode
    );
    assert_eq!(document.point_scan.select_key, "Space");
}

#[test]
fn document_keeps_switch_keys() {
    let document = local().document();
    let keys: Vec<_> = document
        .switches
        .bindings
        .iter()
        .map(|b| b.key.as_str())
        .collect();
    assert_eq!(keys, ["Space", "Enter"]);
}

#[test]
fn document_round_trips_and_plans_nothing() {
    let local = local();
    let value = serde_json::to_value(local.document()).unwrap();
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    assert_eq!(parsed, local.document());
    assert!(plan(&local, parsed).unwrap().is_empty());
}

#[test]
fn parse_rejects_newer_and_malformed_documents() {
    let value = serde_json::to_value(local().document()).unwrap();
    assert_eq!(
        parse(value.clone(), SCHEMA_VERSION + 1),
        Err(ParseError::Newer(SCHEMA_VERSION + 1))
    );
    assert!(matches!(parse(value, 0), Err(ParseError::Invalid(_))));
    assert!(matches!(
        parse(json!({"schemaVersion": SCHEMA_VERSION}), SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    let mut extra = serde_json::to_value(local().document()).unwrap();
    extra["app"]["startWithSystem"] = json!(true);
    assert!(matches!(
        parse(extra, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
}

/// A document as written by version 1, before scanned key repeat.
fn version_one() -> Value {
    written_by(serde_json::to_value(local().document()).unwrap(), 1)
}

#[test]
fn gamepad_switch_keys_sync_as_version_six() {
    // Version 5 installs reject these keys, which is why the schema moved
    // to 6: they see a newer document and ask for an update instead.
    let local = local();
    let mut cloud = local.clone();
    cloud.switches.bindings = vec![
        binding("pad", "Gamepad1", Action::Select),
        binding("next", "Gamepad8", Action::Next),
    ];
    let value = serde_json::to_value(cloud.document()).unwrap();
    assert_eq!(value["schemaVersion"], json!(SCHEMA_VERSION));
    assert_eq!(value["switches"]["bindings"][0]["key"], json!("Gamepad1"));
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    let switches = plan(&local, parsed).unwrap().switches.unwrap();
    assert_eq!(switches, cloud.switches);
    // Button 9 is not a switch, so a document naming it is refused.
    let mut bad = cloud.clone();
    bad.switches.bindings[0].key = "Gamepad9".into();
    let parsed = parse(
        serde_json::to_value(bad.document()).unwrap(),
        SCHEMA_VERSION,
    )
    .unwrap();
    assert!(plan(&local, parsed).is_err());
}

#[test]
fn documents_with_a_rearranged_more_actions_page_still_apply() {
    use crate::builtin_panels::default_document;
    use crate::scan_menu::Kind;
    let local = local();
    let mut cloud = local.clone();
    let mut more = default_document(Kind::Editing);
    more.id = "builtin-more".into();
    cloud.menu_overrides.insert("more".into(), more.clone());
    let parsed = parse(
        serde_json::to_value(cloud.document()).unwrap(),
        SCHEMA_VERSION,
    )
    .unwrap();
    let planned = plan(&local, parsed).unwrap().menu_overrides.unwrap();
    assert_eq!(planned["more"].rows, more.rows);
}

#[test]
fn version_seven_documents_read_with_no_rearranged_menus() {
    use crate::builtin_panels::{default_document, key};
    use crate::scan_menu::Kind;
    let mut local = local();
    let mut home = default_document(Kind::Home);
    home.rows.reverse();
    local.menu_overrides.insert(key(Kind::Home), home.clone());
    let mut cloud = local.clone();
    cloud.menu_overrides.clear();
    let version_seven = written_by(serde_json::to_value(cloud.document()).unwrap(), 7);
    assert!(version_seven.get("menuOverrides").is_none());
    let parsed = parse(version_seven.clone(), 7).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert!(parsed.menu_overrides.is_empty());
    // Applying it resets this computer's menus, as a reset elsewhere would.
    assert_eq!(
        plan(&local, parsed).unwrap().menu_overrides,
        Some(BTreeMap::new())
    );
    let mut early = version_seven.clone();
    early["menuOverrides"] = json!({});
    assert!(matches!(parse(early, 7), Err(ParseError::Invalid(_))));
    let mut missing = version_seven.clone();
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // A rearranged menu syncs, and a menu that may not change refuses the document.
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(value["menuOverrides"]["home"]["id"], json!("builtin-home"));
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    assert!(plan(&local, parsed).unwrap().is_empty());
    let mut bad = local.clone();
    bad.menu_overrides
        .insert("shutDown".into(), default_document(Kind::ShutDown));
    let parsed = parse(
        serde_json::to_value(bad.document()).unwrap(),
        SCHEMA_VERSION,
    )
    .unwrap();
    assert!(plan(&local, parsed).is_err());
}

#[test]
fn version_six_documents_read_with_no_custom_panels() {
    let local = local();
    let mut cloud = local.clone();
    cloud.panels.clear();
    let version_six = written_by(serde_json::to_value(cloud.document()).unwrap(), 6);
    assert!(version_six.get("panels").is_none());
    let parsed = parse(version_six.clone(), 6).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert!(parsed.panels.is_empty());
    // Applying it removes this computer's panels, as a deletion elsewhere would.
    let planned = plan(&local, parsed).unwrap();
    assert_eq!(planned.panels, Some(Vec::new()));

    // A version 6 document never carried panels, and a version 7 one must.
    let mut early = version_six.clone();
    early["panels"] = json!([]);
    assert!(matches!(parse(early, 6), Err(ParseError::Invalid(_))));
    let mut missing = version_six.clone();
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));

    // Panels sync with their tiles, and an invalid panel refuses the document.
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(
        value["panels"][0]["rows"][0][1]["action"]["type"],
        json!("text")
    );
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    assert!(plan(&local, parsed).unwrap().is_empty());
    let mut bad = local.clone();
    bad.panels[0].rows.clear();
    let parsed = parse(
        serde_json::to_value(bad.document()).unwrap(),
        SCHEMA_VERSION,
    )
    .unwrap();
    assert!(plan(&local, parsed).is_err());
}

#[test]
fn version_four_documents_read_with_panels_matching_the_system() {
    use crate::scan_preferences::PanelTheme;
    let mut local = local();
    local.point_scan.scan_preferences.panel_theme = PanelTheme::Light;
    let mut cloud = local.clone();
    cloud.point_scan.scan_preferences.panel_theme = PanelTheme::System;
    let version_four = written_by(serde_json::to_value(cloud.document()).unwrap(), 4);
    assert!(version_four["pointScan"]["scanPreferences"]
        .get("panelTheme")
        .is_none());
    let parsed = parse(version_four.clone(), 4).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(
        parsed.point_scan.scan_preferences.panel_theme,
        PanelTheme::System
    );
    let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
    assert_eq!(point_scan.scan_preferences.panel_theme, PanelTheme::System);

    // Version 4 never had a panel theme, and a version 5 document needs it.
    let mut early = version_four.clone();
    early["pointScan"]["scanPreferences"]["panelTheme"] = json!("light");
    assert!(matches!(parse(early, 4), Err(ParseError::Invalid(_))));
    let mut missing = version_four.clone();
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // Every older version still climbs to the current one.
    for version in 1..SCHEMA_VERSION {
        let old = written_by(serde_json::to_value(cloud.document()).unwrap(), version);
        let parsed = parse(old, version).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(
            parsed.point_scan.scan_preferences.panel_theme,
            PanelTheme::System
        );
    }
    // A merge base saved by version 4 still loads.
    let base: Document = serde_json::from_value(written_by(
        serde_json::to_value(local.document()).unwrap(),
        4,
    ))
    .unwrap();
    assert_eq!(
        base.point_scan.scan_preferences.panel_theme,
        PanelTheme::System
    );

    // A version 5 document climbs unchanged: nothing was added in 6.
    let version_five = written_by(serde_json::to_value(cloud.document()).unwrap(), 5);
    let parsed = parse(version_five.clone(), 5).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.switches, cloud.switches);
    assert!(matches!(
        parse(version_five, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));

    // A chosen theme syncs, and an unknown one is refused rather than reset.
    let mut chosen = serde_json::to_value(cloud.document()).unwrap();
    chosen["pointScan"]["scanPreferences"]["panelTheme"] = json!("dark");
    let parsed = parse(chosen.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(
        parsed.point_scan.scan_preferences.panel_theme,
        PanelTheme::Dark
    );
    chosen["pointScan"]["scanPreferences"]["panelTheme"] = json!("sepia");
    assert!(matches!(
        parse(chosen, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
}

#[test]
fn version_three_documents_read_with_the_radar_following_line_speed() {
    let mut local = local();
    local.point_scan.radar_speed = Some(4);
    let mut cloud = local.clone();
    cloud.point_scan.radar_speed = None;
    let version_three = written_by(serde_json::to_value(cloud.document()).unwrap(), 3);
    let parsed = parse(version_three.clone(), 3).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.point_scan.radar_speed, None);
    let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
    assert_eq!(point_scan.radar_speed, None);

    // Version 3 never had a radar speed, and a version 4 document needs it.
    let mut early = version_three.clone();
    early["pointScan"]["radarSpeed"] = json!(4);
    assert!(matches!(parse(early, 3), Err(ParseError::Invalid(_))));
    let mut missing = version_three;
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // A merge base saved by version 3 still loads.
    let base: Document = serde_json::from_value(written_by(
        serde_json::to_value(local.document()).unwrap(),
        3,
    ))
    .unwrap();
    assert_eq!(base.point_scan.radar_speed, None);

    // A chosen radar speed syncs, and an invalid one is refused.
    let mut chosen = serde_json::to_value(cloud.document()).unwrap();
    chosen["pointScan"]["radarSpeed"] = json!(0);
    let parsed = parse(chosen.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(parsed.point_scan.radar_speed, Some(0));
    chosen["pointScan"]["radarSpeed"] = json!(5);
    let parsed = parse(chosen, SCHEMA_VERSION).unwrap();
    assert!(plan(&local, parsed).is_err());
}

#[test]
fn version_two_documents_read_with_row_groups_off_and_inherited() {
    let mut local = local();
    local.point_scan.scan_preferences.row_groups = true;
    local.point_scan.scan_preferences.keyboard.row_groups = Some(true);
    let mut cloud = local.clone();
    cloud.point_scan.scan_preferences = Default::default();
    let version_two = written_by(serde_json::to_value(cloud.document()).unwrap(), 2);
    let parsed = parse(version_two.clone(), 2).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    let preferences = &parsed.point_scan.scan_preferences;
    assert!(!preferences.row_groups);
    for area in [
        &preferences.point,
        &preferences.menu,
        &preferences.keyboard,
        &preferences.mouse,
    ] {
        assert_eq!(area.row_groups, None);
    }
    let point_scan = plan(&local, parsed).unwrap().point_scan.unwrap();
    assert_eq!(point_scan.scan_preferences, Default::default());

    // Version 2 never had row groups, and a version 3 document needs them.
    let mut early = version_two.clone();
    early["pointScan"]["scanPreferences"]["menu"]["rowGroups"] = json!(true);
    assert!(matches!(parse(early, 2), Err(ParseError::Invalid(_))));
    let mut missing = version_two;
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // A merge base saved by version 2 still loads.
    let base: Document = serde_json::from_value(written_by(
        serde_json::to_value(local.document()).unwrap(),
        2,
    ))
    .unwrap();
    assert!(!base.point_scan.scan_preferences.row_groups);
}

#[test]
fn synced_scan_preferences_are_deliberate() {
    // Changing scan_preferences::Preferences changes what syncs. Update
    // this list and bump SCHEMA_VERSION together.
    let value = serde_json::to_value(local().document()).unwrap();
    let preferences = &value["pointScan"]["scanPreferences"];
    let keys = |value: &Value| {
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort_unstable();
        keys
    };
    assert_eq!(
        keys(preferences),
        [
            "direction",
            "firstItemMs",
            "groupRatePercent",
            "keyboard",
            "menu",
            "mouse",
            "nextScan",
            "panelColors",
            "panelSize",
            "panelTheme",
            "passLimit",
            "pattern",
            "point",
            "rowGroups",
            "startFrom",
            "thickness",
        ]
    );
    for area in ["point", "menu", "keyboard", "mouse"] {
        assert_eq!(
            keys(&preferences[area]),
            [
                "automatic",
                "color",
                "direction",
                "intervalMs",
                "nextScan",
                "passLimit",
                "pattern",
                "rowGroups",
                "startFrom",
                "thickness",
            ]
        );
    }
}

#[test]
fn version_one_documents_read_with_scanned_key_repeat_off() {
    let mut local = local();
    local.settings.scan_key_repeat_enabled = true;
    let parsed = parse(version_one(), 1).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert!(!parsed.app.scan_key_repeat_enabled);
    let settings = plan(&local, parsed).unwrap().settings.unwrap();
    assert!(!settings.scan_key_repeat_enabled);

    // Version 1 never had the field, and a version 2 document needs it.
    let mut early = version_one();
    early["app"]["scanKeyRepeatEnabled"] = json!(true);
    assert!(matches!(parse(early, 1), Err(ParseError::Invalid(_))));
    let mut missing = version_one();
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing.clone(), SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    assert!(matches!(parse(missing, 1), Err(ParseError::Invalid(_))));
}

#[test]
fn version_thirteen_documents_read_with_smooth_movement() {
    use crate::mouse_repeat::{BlockSize, MouseMovement};
    let mut local = local();
    local.settings.mouse_movement = MouseMovement::Blocks;
    local.settings.mouse_block_size = BlockSize::Large;
    let current = serde_json::to_value(local.document()).unwrap();
    let older = written_by(current, 13);
    let parsed = parse(older.clone(), 13).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.mouse_movement, MouseMovement::Smooth);
    assert_eq!(parsed.app.mouse_block_size, BlockSize::Medium);
    let settings = plan(&local, parsed).unwrap().settings.unwrap();
    assert_eq!(settings.mouse_movement, MouseMovement::Smooth);

    // Version 13 never had the fields, and version 14 needs them.
    let mut early = older.clone();
    early["app"]["mouseMovement"] = json!("blocks");
    assert!(matches!(parse(early, 13), Err(ParseError::Invalid(_))));
    let mut missing = older;
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // An older merge base without the fields still loads.
    let base: Document = serde_json::from_value(written_by(
        serde_json::to_value(local.document()).unwrap(),
        13,
    ))
    .unwrap();
    assert_eq!(base.app.mouse_movement, MouseMovement::Smooth);
}

#[test]
fn block_movement_round_trips_in_the_current_version() {
    use crate::mouse_repeat::{BlockSize, MouseMovement};
    let mut local = local();
    local.settings.mouse_movement = MouseMovement::Blocks;
    local.settings.mouse_block_size = BlockSize::Small;
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(value["app"]["mouseMovement"], json!("blocks"));
    assert_eq!(value["app"]["mouseBlockSize"], json!("small"));
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    assert_eq!(parsed.app.mouse_movement, MouseMovement::Blocks);
    assert_eq!(parsed.app.mouse_block_size, BlockSize::Small);
}

#[test]
fn version_fourteen_documents_read_with_scan_speed_blocks() {
    use crate::mouse_repeat::{BlockSize, MouseMovement};
    let mut local = local();
    local.settings.mouse_movement = MouseMovement::Blocks;
    local.settings.mouse_block_size = BlockSize::Large;
    local.settings.mouse_block_interval_ms = 2000;
    let current = serde_json::to_value(local.document()).unwrap();
    let older = written_by(current, 14);
    let parsed = parse(older.clone(), 14).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.mouse_movement, MouseMovement::Blocks);
    assert_eq!(parsed.app.mouse_block_size, BlockSize::Large);
    assert_eq!(parsed.app.mouse_block_interval_ms, 0);
    let settings = plan(&local, parsed).unwrap().settings.unwrap();
    assert_eq!(settings.mouse_block_interval_ms, 0);

    // Version 14 never had the field, and version 15 needs it.
    let mut early = older.clone();
    early["app"]["mouseBlockIntervalMs"] = json!(1000);
    assert!(matches!(parse(early, 14), Err(ParseError::Invalid(_))));
    let mut missing = older;
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // An older merge base without the field still loads.
    let base: Document = serde_json::from_value(written_by(
        serde_json::to_value(local.document()).unwrap(),
        14,
    ))
    .unwrap();
    assert_eq!(base.app.mouse_block_interval_ms, 0);
}

#[test]
fn version_sixteen_documents_keep_their_dwell_and_key_repeat_timing() {
    let mut local = local();
    local.settings.dwell_click_delay_ms = 4000;
    local.settings.key_repeat_initial_delay_ms = 250;
    local.settings.key_repeat_interval_ms = 100;
    let older = written_by(serde_json::to_value(local.document()).unwrap(), 16);
    assert_eq!(older["schemaVersion"], json!(16));
    let parsed = parse(older.clone(), 16).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.dwell_click_delay_ms, 4000);
    assert_eq!(parsed.app.key_repeat_initial_delay_ms, 250);
    assert_eq!(parsed.app.key_repeat_interval_ms, 100);

    // Version 16 offered only its fixed choices.
    for (field, ms) in [
        ("dwellClickDelayMs", 2500),
        ("keyRepeatInitialDelayMs", 750),
        ("keyRepeatIntervalMs", 150),
    ] {
        let mut odd = older.clone();
        odd["app"][field] = json!(ms);
        assert!(
            matches!(parse(odd, 16), Err(ParseError::Invalid(_))),
            "{field}"
        );
    }
}

#[test]
fn stepped_dwell_and_key_repeat_timing_syncs_in_the_current_version() {
    let mut local = local();
    local.settings.dwell_click_delay_ms = 2500;
    local.settings.key_repeat_initial_delay_ms = 750;
    local.settings.key_repeat_interval_ms = 150;
    let value = serde_json::to_value(local.document()).unwrap();
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    let settings = plan(
        &super::Local {
            settings: Default::default(),
            ..local.clone()
        },
        parsed,
    )
    .unwrap()
    .settings
    .unwrap();
    assert_eq!(settings.dwell_click_delay_ms, 2500);
    assert_eq!(settings.key_repeat_initial_delay_ms, 750);
    assert_eq!(settings.key_repeat_interval_ms, 150);

    for (field, ms) in [
        ("dwellClickDelayMs", 10_100),
        ("dwellClickDelayMs", 50),
        ("keyRepeatInitialDelayMs", 2050),
        ("keyRepeatIntervalMs", 1050),
        ("keyRepeatIntervalMs", 125),
    ] {
        let mut odd = value.clone();
        odd["app"][field] = json!(ms);
        let parsed = parse(odd, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).is_err(), "{field} {ms}");
    }
}

#[test]
fn version_seventeen_documents_keep_their_mouse_repeat_timing() {
    let mut local = local();
    local.settings.move_repeat_interval_ms = 500;
    local.settings.scroll_repeat_interval_ms = 1000;
    local.settings.mouse_repeat_acceleration_duration_ms = 2000;
    let older = written_by(serde_json::to_value(local.document()).unwrap(), 17);
    assert_eq!(older["schemaVersion"], json!(17));
    let parsed = parse(older.clone(), 17).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.move_repeat_interval_ms, 500);
    assert_eq!(parsed.app.scroll_repeat_interval_ms, 1000);
    assert_eq!(parsed.app.mouse_repeat_acceleration_duration_ms, 2000);

    // Version 17 offered only its fixed choices.
    for (field, ms) in [
        ("moveRepeatIntervalMs", 150),
        ("scrollRepeatIntervalMs", 2000),
        ("mouseRepeatAccelerationDurationMs", 300),
    ] {
        let mut odd = older.clone();
        odd["app"][field] = json!(ms);
        assert!(
            matches!(parse(odd, 17), Err(ParseError::Invalid(_))),
            "{field}"
        );
    }
}

#[test]
fn stepped_mouse_repeat_timing_syncs_in_the_current_version() {
    let mut local = local();
    local.settings.move_repeat_interval_ms = 150;
    local.settings.scroll_repeat_interval_ms = 2000;
    local.settings.mouse_repeat_acceleration_duration_ms = 300;
    let value = serde_json::to_value(local.document()).unwrap();
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    let settings = plan(
        &super::Local {
            settings: Default::default(),
            ..local.clone()
        },
        parsed,
    )
    .unwrap()
    .settings
    .unwrap();
    assert_eq!(settings.move_repeat_interval_ms, 150);
    assert_eq!(settings.scroll_repeat_interval_ms, 2000);
    assert_eq!(settings.mouse_repeat_acceleration_duration_ms, 300);

    for (field, ms) in [
        ("moveRepeatIntervalMs", 2050),
        ("moveRepeatIntervalMs", 125),
        ("scrollRepeatIntervalMs", 50),
        ("mouseRepeatAccelerationDurationMs", 3100),
        ("mouseRepeatAccelerationDurationMs", 250),
    ] {
        let mut odd = value.clone();
        odd["app"][field] = json!(ms);
        let parsed = parse(odd, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).is_err(), "{field} {ms}");
    }
}

#[test]
fn version_eighteen_documents_keep_their_press_timing() {
    let mut local = local();
    local.switches.accept_ms = 300;
    local.switches.ignore_repeat_ms = 2000;
    let older = written_by(serde_json::to_value(local.document()).unwrap(), 18);
    assert_eq!(older["schemaVersion"], json!(18));
    let parsed = parse(older.clone(), 18).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.switches.accept_ms, 300);
    assert_eq!(parsed.switches.ignore_repeat_ms, 2000);

    // Press timing that is off is left out, as version 18 wrote it.
    let mut off = older.clone();
    let switches = off["switches"].as_object_mut().unwrap();
    switches.remove("acceptMs");
    switches.remove("ignoreRepeatMs");
    let parsed = parse(off, 18).unwrap();
    assert_eq!(parsed.switches.accept_ms, 0);
    assert_eq!(parsed.switches.ignore_repeat_ms, 0);

    // Version 18 offered only its fixed choices.
    for (field, ms) in [("acceptMs", 150), ("ignoreRepeatMs", 750)] {
        let mut odd = older.clone();
        odd["switches"][field] = json!(ms);
        assert!(
            matches!(parse(odd, 18), Err(ParseError::Invalid(_))),
            "{field}"
        );
    }
}

#[test]
fn stepped_press_timing_syncs_in_the_current_version() {
    let mut local = local();
    local.switches.accept_ms = 150;
    local.switches.ignore_repeat_ms = 2750;
    let value = serde_json::to_value(local.document()).unwrap();
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    let switches = plan(
        &super::Local {
            switches: switches::Settings {
                bindings: local.switches.bindings.clone(),
                ..switches::Settings::default()
            },
            ..local.clone()
        },
        parsed,
    )
    .unwrap()
    .switches
    .unwrap();
    assert_eq!(switches.accept_ms, 150);
    assert_eq!(switches.ignore_repeat_ms, 2750);

    for (field, ms) in [
        ("acceptMs", 2050),
        ("acceptMs", 125),
        ("ignoreRepeatMs", 3050),
        ("ignoreRepeatMs", 260),
    ] {
        let mut odd = value.clone();
        odd["switches"][field] = json!(ms);
        let parsed = parse(odd, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).is_err(), "{field} {ms}");
    }
}

#[test]
fn version_nineteen_documents_keep_their_first_item_time() {
    let mut local = local();
    local.point_scan.scan_preferences.first_item_ms = 1500;
    let older = written_by(serde_json::to_value(local.document()).unwrap(), 19);
    assert_eq!(older["schemaVersion"], json!(19));
    let parsed = parse(older.clone(), 19).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.point_scan.scan_preferences.first_item_ms, 1500);

    // Version 19 offered only its fixed choices.
    let mut odd = older.clone();
    odd["pointScan"]["scanPreferences"]["firstItemMs"] = json!(300);
    assert!(matches!(parse(odd, 19), Err(ParseError::Invalid(_))));
}

#[test]
fn stepped_first_item_time_syncs_in_the_current_version() {
    let mut local = local();
    local.point_scan.scan_preferences.first_item_ms = 300;
    let value = serde_json::to_value(local.document()).unwrap();
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(parsed.point_scan.scan_preferences.first_item_ms, 300);
    let point_scan = plan(
        &super::Local {
            point_scan: point_scan::Config {
                scan_preferences: Default::default(),
                ..local.point_scan.clone()
            },
            ..local.clone()
        },
        parsed,
    )
    .unwrap()
    .point_scan
    .unwrap();
    assert_eq!(point_scan.scan_preferences.first_item_ms, 300);

    // Saved preferences read a time outside the range as off, so a
    // document holding one no longer round-trips and is rejected rather
    // than turning the time off.
    for ms in [3100, 250] {
        let mut odd = value.clone();
        odd["pointScan"]["scanPreferences"]["firstItemMs"] = json!(ms);
        assert!(
            matches!(parse(odd, SCHEMA_VERSION), Err(ParseError::Invalid(_))),
            "{ms}"
        );
    }
}

#[test]
fn version_fifteen_documents_keep_their_block_speed() {
    let mut local = local();
    local.settings.mouse_block_interval_ms = 1500;
    let older = written_by(serde_json::to_value(local.document()).unwrap(), 15);
    assert_eq!(older["schemaVersion"], json!(15));
    let parsed = parse(older.clone(), 15).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.mouse_block_interval_ms, 1500);

    // Version 15 offered only its five times and Scan speed.
    let mut odd = older;
    odd["app"]["mouseBlockIntervalMs"] = json!(1200);
    assert!(matches!(parse(odd, 15), Err(ParseError::Invalid(_))));

    // A version 14 document still reaches the current version.
    let oldest = written_by(serde_json::to_value(local.document()).unwrap(), 14);
    let parsed = parse(oldest, 14).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    assert_eq!(parsed.app.mouse_block_interval_ms, 0);
}

#[test]
fn any_tenth_of_a_second_block_speed_syncs_in_the_current_version() {
    let mut local = local();
    for ms in [100, 1200, 10_000] {
        local.settings.mouse_block_interval_ms = ms;
        let value = serde_json::to_value(local.document()).unwrap();
        assert_eq!(value["schemaVersion"], json!(SCHEMA_VERSION));
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        let mut other = local.clone();
        other.settings.mouse_block_interval_ms = 0;
        assert_eq!(
            plan(&other, parsed)
                .unwrap()
                .settings
                .map(|s| s.mouse_block_interval_ms),
            Some(ms)
        );
    }
    for ms in [50, 1250, 10_100] {
        local.settings.mouse_block_interval_ms = 0;
        let mut value = serde_json::to_value(local.document()).unwrap();
        value["app"]["mouseBlockIntervalMs"] = json!(ms);
        let parsed = parse(value, SCHEMA_VERSION).unwrap();
        assert!(plan(&local, parsed).is_err(), "{ms}");
    }
}

#[test]
fn block_speed_round_trips_and_rejects_unknown_speeds() {
    let mut local = local();
    local.settings.mouse_block_interval_ms = 1500;
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(value["app"]["mouseBlockIntervalMs"], json!(1500));
    let parsed = parse(value.clone(), SCHEMA_VERSION).unwrap();
    assert_eq!(parsed.app.mouse_block_interval_ms, 1500);
    let mut other = local.clone();
    other.settings.mouse_block_interval_ms = 0;
    assert_eq!(
        plan(&other, parsed)
            .unwrap()
            .settings
            .map(|s| s.mouse_block_interval_ms),
        Some(1500)
    );

    let mut odd = value;
    odd["app"]["mouseBlockIntervalMs"] = json!(1234);
    let parsed = parse(odd, SCHEMA_VERSION).unwrap();
    assert!(plan(&local, parsed).is_err());
}

#[test]
fn a_merge_base_saved_by_version_one_still_loads() {
    let base: Document = serde_json::from_value(version_one()).unwrap();
    assert!(!base.app.scan_key_repeat_enabled);
}

#[test]
fn plan_keeps_local_only_settings() {
    let local = local();
    let mut incoming = local.document();
    incoming.app.dwell_click_enabled = true;
    let plan = plan(&local, incoming).unwrap();
    let settings = plan.settings.unwrap();
    assert!(settings.dwell_click_enabled);
    assert!(settings.start_with_system);
    assert!(settings.share_diagnostics);
    assert!(!settings.auto_approve_account_devices);
    assert_eq!(settings.text_size_percent, 150);
    assert!(plan.switches.is_none() && plan.point_scan.is_none() && plan.remote.is_none());
}

#[test]
fn plan_keeps_runtime_point_scan_state() {
    let local = local();
    let mut incoming = local.document();
    incoming.point_scan.speed = 4;
    let config = plan(&local, incoming).unwrap().point_scan.unwrap();
    assert_eq!(config.speed, 4);
    assert_eq!(config.control_mode, point_scan::ControlMode::Mouse);
    assert_eq!(config.select_key, "F1");
}

#[test]
fn plan_separates_keyboard_layout() {
    let local = local();
    let mut incoming = local.document();
    incoming.point_scan.keyboard_layout = point_scan::KeyboardLayout::SimpleQwerty;
    let plan = plan(&local, incoming).unwrap();
    assert_eq!(
        plan.keyboard_layout,
        Some(point_scan::KeyboardLayout::SimpleQwerty)
    );
    assert!(plan.point_scan.is_none());
}

#[test]
fn plan_keeps_local_remote_revision() {
    let local = local();
    let mut incoming = local.document();
    incoming.remote_switches[6].press_action = Some(Action::Select);
    let remote = plan(&local, incoming).unwrap().remote.unwrap();
    assert_eq!(remote.revision, 7);
    assert_eq!(remote.slots[6].press_action, Some(Action::Select));
}

#[test]
fn plan_rejects_any_invalid_section() {
    let local = local();
    let invalid: [fn(&mut Document); 6] = [
        |d| d.app.pointer_scale_percent = 3,
        |d| d.app.cursor_overlay_color = "purple".into(),
        |d| d.switches.bindings[1].key = "Space".into(),
        |d| d.switches.hold_interval_ms = 10,
        |d| d.point_scan.grid_size = 99,
        |d| d.remote_switches.truncate(3),
    ];
    for corrupt in invalid {
        let mut incoming = local.document();
        corrupt(&mut incoming);
        assert!(plan(&local, incoming).is_err());
    }
}

#[test]
fn plan_rejects_invalid_profiles() {
    let local = local();
    let invalid: [fn(&mut Document); 4] = [
        |d| d.profiles[0].built_in = true,
        |d| d.profiles[0].id = "not-a-uuid".into(),
        |d| {
            let mut other = custom_profile(PROFILE_B, "DESK", "Tab");
            other.version = 1;
            d.profiles.push(other);
        },
        |d| {
            let copy = d.profiles[0].clone();
            d.profiles.push(copy);
        },
    ];
    for corrupt in invalid {
        let mut incoming = local.document();
        corrupt(&mut incoming);
        assert!(plan(&local, incoming).is_err());
    }
    let mut incoming = local.document();
    incoming.profiles = (0..33)
        .map(|_| custom_profile(&uuid::Uuid::new_v4().to_string(), "x", "Space"))
        .enumerate()
        .map(|(index, mut profile)| {
            profile.name = format!("Profile {index}");
            profile
        })
        .collect();
    assert!(plan(&local, incoming).is_err());
}

#[test]
fn changed_profiles_get_a_newer_version() {
    let local = local();
    let mut incoming = local.document();
    incoming.profiles[0].bindings[0].value = Some("Enter".into());
    incoming.profiles[0].version = 1;
    incoming
        .profiles
        .push(custom_profile(PROFILE_B, "Laptop", "Tab"));
    let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
    assert!(profiles[0].built_in, "built-ins stay first");
    let desk = profiles.iter().find(|p| p.id == PROFILE_A).unwrap();
    assert_eq!(desk.version, 2);
    assert_eq!(desk.bindings[0].value.as_deref(), Some("Enter"));
    assert!(profiles.iter().any(|p| p.id == PROFILE_B));
}

#[test]
fn unchanged_profiles_keep_their_version() {
    let mut local = local();
    local.profiles[1].version = 5;
    let mut incoming = local.document();
    incoming.profiles[0].version = 2;
    assert!(plan(&local, incoming).unwrap().profiles.is_none());
}

#[test]
fn removed_profiles_are_dropped() {
    let local = local();
    let mut incoming = local.document();
    incoming.profiles.clear();
    let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
    assert!(profiles.iter().all(|profile| profile.built_in));
    assert!(!profiles.is_empty());
}

#[test]
fn reserved_names_cover_every_platform_built_in() {
    let mut reserved: Vec<_> = crate::state::reserved_profile_names().collect();
    let mut built_in: Vec<_> = built_in_profiles(true)
        .into_iter()
        .map(|profile| profile.name)
        .collect();
    reserved.sort_unstable();
    built_in.sort_unstable();
    assert_eq!(reserved, built_in);
}

#[test]
fn profile_named_after_another_platforms_built_in_is_renamed() {
    // A Mac has no built-in "Grid 3", so a custom one could exist there.
    let local = local();
    let mut incoming = local.document();
    incoming.profiles[0].name = "grid 3".into();
    let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
    let desk = profiles.iter().find(|p| p.id == PROFILE_A).unwrap();
    assert_eq!(desk.name, "grid 3 (custom)");
}

#[test]
fn reserved_rename_skips_names_already_in_use() {
    // Both were valid custom names on a Mac before names were reserved.
    let local = local();
    let mut incoming = local.document();
    incoming.profiles[0].name = "Grid 3".into();
    incoming
        .profiles
        .push(custom_profile(PROFILE_B, "Grid 3 (custom)", "Tab"));
    let profiles = plan(&local, incoming).unwrap().profiles.unwrap();
    let name = |id| &profiles.iter().find(|p| p.id == id).unwrap().name;
    assert_eq!(name(PROFILE_A), "Grid 3 (custom 2)");
    assert_eq!(name(PROFILE_B), "Grid 3 (custom)");
}

#[test]
fn load_and_merge_rename_the_same_way() {
    // An install renames on load; its next document must then plan
    // nothing on another install that renamed the same profile.
    let mut renamed = local();
    renamed.profiles[1].name = "Grid 3".into();
    crate::state::rename_reserved_profiles(&mut renamed.profiles);
    assert_eq!(renamed.profiles[1].name, "Grid 3 (custom)");
    assert!(renamed.profiles[0].built_in && renamed.profiles[0].name == "Generic keyboard");

    let mut legacy = renamed.document();
    legacy.profiles[0].name = "Grid 3".into();
    assert!(plan(&renamed, legacy).unwrap().profiles.is_none());
}

#[test]
fn profile_names_compare_like_local_saves() {
    // save_switch_profile compares ASCII case-insensitively, so both of
    // these can exist on one install and must sync.
    let local = local();
    let mut incoming = local.document();
    incoming.profiles[0].name = "Été".into();
    let mut second = custom_profile(PROFILE_B, "été", "Tab");
    second.version = 1;
    incoming.profiles.push(second);
    assert!(plan(&local, incoming).unwrap().profiles.is_some());
}

#[test]
fn parse_rejects_lossy_point_scan_sections() {
    let document = serde_json::to_value(local().document()).unwrap();
    let lossy: [fn(&mut Value); 4] = [
        // Partial section: missing fields would silently become defaults.
        |v| v["pointScan"] = json!({}),
        // A field from a newer build would be dropped without a trace.
        |v| v["pointScan"]["futureSetting"] = json!(true),
        // An unknown scan preference value resets all scan preferences.
        |v| v["pointScan"]["scanPreferences"] = json!("garbage"),
        |v| v["pointScan"]["scanPreferences"]["pattern"] = json!("spiral"),
    ];
    for corrupt in lossy {
        let mut value = document.clone();
        corrupt(&mut value);
        assert!(
            matches!(
                parse(value.clone(), SCHEMA_VERSION),
                Err(ParseError::Invalid(_))
            ),
            "accepted {value}"
        );
    }
}

#[test]
fn synced_point_scan_fields_are_deliberate() {
    // Changing point_scan::Config changes what syncs. Update this list
    // and bump SCHEMA_VERSION together.
    let value = serde_json::to_value(local().document()).unwrap();
    let mut fields: Vec<_> = value["pointScan"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "autoSelectDelayMs",
            "autoSelectEnabled",
            "automatic",
            "backKey",
            "blockIntervalMs",
            "controlMode",
            "gridSize",
            "keyboardLayout",
            "keyboardWaitAfterTyping",
            "mode",
            "mouseRepeatStopEdge",
            "nextKey",
            "panelAvoidsPointer",
            "pauseKey",
            "radarSpeed",
            "scanPreferences",
            "scannerColor",
            "selectKey",
            "speed",
            "startWith",
            "wordPrediction",
        ]
    );
}

#[test]
fn version_twenty_one_documents_read_with_built_in_panel_colours_on() {
    let mut local = local();
    local.point_scan.scan_preferences.panel_colors = false;
    let cloud = local.clone();
    let version_twenty_one = written_by(serde_json::to_value(cloud.document()).unwrap(), 21);
    assert!(version_twenty_one
        .pointer("/pointScan/scanPreferences/panelColors")
        .is_none());
    let parsed = parse(version_twenty_one.clone(), 21).unwrap();
    assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    // Version 21 had no choice, so its computers showed the colours.
    assert!(parsed.point_scan.scan_preferences.panel_colors);
    let mut early = version_twenty_one.clone();
    early["pointScan"]["scanPreferences"]["panelColors"] = json!(false);
    assert!(matches!(parse(early, 21), Err(ParseError::Invalid(_))));
    let mut missing = version_twenty_one;
    missing["schemaVersion"] = json!(SCHEMA_VERSION);
    assert!(matches!(
        parse(missing, SCHEMA_VERSION),
        Err(ParseError::Invalid(_))
    ));
    // The choice syncs in the current version.
    let value = serde_json::to_value(local.document()).unwrap();
    assert_eq!(
        value["pointScan"]["scanPreferences"]["panelColors"],
        json!(false)
    );
    let parsed = parse(value, SCHEMA_VERSION).unwrap();
    assert!(!parsed.point_scan.scan_preferences.panel_colors);
}
