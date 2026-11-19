use super::*;

// Existing Android and desktop clients read these lists, so their exact
// contents and order are part of the protocol. Change them only with a
// matching client change.
#[test]
fn advertised_command_lists_are_unchanged() {
    let mut supported = vec![
        "mouse.move",
        "mouse.click",
        "mouse.doubleClick",
        "mouse.rightClick",
        "mouse.scroll",
        "mouse.dragStart",
        "mouse.dragEnd",
        "mouse.repeat.start",
        "mouse.repeat.stop",
        "connection.ping",
        "pointer.profile",
        "pointer.speed.set",
        "keyboard.key",
        "keyboard.modifierDown",
        "keyboard.modifierUp",
        "keyboard.shortcut",
        "keyboard.typeText",
        "keyboard.textStream.open",
        "keyboard.textStream.char",
        "keyboard.textStream.chunk",
        "keyboard.textStream.key",
        "keyboard.textStream.close",
        "media.control",
        "window.control",
        "switch.profile.list",
        "switch.session.start",
        "switch.edge",
        "switch.sync",
        "switch.session.stop",
        "connection.disconnecting",
    ];
    let mut with_display = supported.clone();
    with_display.push("pointer.display.move");
    if cfg!(target_os = "windows") {
        supported.extend(["grid.switch.set", "grid.switch.sync"]);
        with_display.extend(["grid.switch.set", "grid.switch.sync"]);
    }
    assert_eq!(supported_commands(false), supported);
    assert_eq!(supported_commands(true), with_display);
    assert_eq!(
        no_ack_commands(),
        [
            "mouse.move",
            "mouse.click",
            "mouse.doubleClick",
            "mouse.rightClick",
            "mouse.scroll",
            "mouse.dragStart",
            "mouse.dragEnd",
            "mouse.repeat.start",
            "mouse.repeat.stop",
            "keyboard.key",
            "keyboard.modifierDown",
            "keyboard.modifierUp",
            "keyboard.shortcut",
            "keyboard.typeText",
            "keyboard.textStream.char",
            "keyboard.textStream.chunk",
            "keyboard.textStream.key",
            "media.control",
            "window.control",
            "grid.switch.set",
            "switch.edge",
        ]
    );
}

#[test]
fn desktop_and_locked_command_sets_are_unchanged() {
    let desktop = [
        "mouse.scroll",
        "mouse.repeat.start",
        "mouse.repeat.stop",
        "mouse.dragStart",
        "mouse.dragEnd",
        "keyboard.key",
        "keyboard.modifierDown",
        "keyboard.modifierUp",
        "keyboard.shortcut",
        "keyboard.textStream.open",
        "keyboard.textStream.char",
        "keyboard.textStream.chunk",
        "keyboard.textStream.key",
        "keyboard.textStream.close",
        "media.control",
        "window.control",
        "grid.switch.set",
        "grid.switch.sync",
        "switch.profile.list",
        "switch.session.start",
        "switch.edge",
        "switch.sync",
        "switch.session.stop",
        "pointer.display.move",
        "pointer.speed.set",
        "connection.disconnecting",
    ];
    let locked = [
        "connection.ping",
        "pointer.profile",
        "switch.session.stop",
        "switch.profile.list",
        "mouse.repeat.stop",
        "mouse.dragEnd",
        "keyboard.modifierUp",
        "connection.disconnecting",
    ];
    let mut names: Vec<&str> = supported_commands(true);
    names.extend(["grid.switch.set", "grid.switch.sync", "unknown.command", ""]);
    for name in names {
        assert_eq!(is_desktop_command(name), desktop.contains(&name), "{name}");
        assert_eq!(allowed_when_locked(name), locked.contains(&name), "{name}");
    }
}

#[test]
fn scanning_catalog_opt_in_preserves_existing_requests() {
    assert!(valid_desktop_payload("switch.profile.list", &json!({})));
    assert!(valid_desktop_payload(
        "switch.profile.list",
        &json!({"includeScanning":true})
    ));
    assert!(!valid_desktop_payload(
        "switch.profile.list",
        &json!({"includeScanning":"yes"})
    ));
    let legacy: Value = serde_json::from_str(&switch_profile_catalog_response(
        "id",
        &crate::state::built_in_profiles(true),
    ))
    .unwrap();
    assert!(legacy["payload"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["kind"] != "scanning"));
}
#[test]
fn bluetooth_status_advertises_platform_without_a_version_bump() {
    for platform in ["windows", "macos"] {
        let payload = bluetooth_status_payload("Switchify PC", "desktop-1", platform).unwrap();
        let status: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(status["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(status["displayName"], "Switchify PC");
        assert_eq!(status["desktopId"], "desktop-1");
        assert_eq!(status["platform"], platform);
    }
}

const NOW: i64 = 1_724_000_000_000;
const TOKEN: &str = "shared-token";

fn pairing_request(id: &str, device_id: &str, device_name: &str, nonce: &str) -> Value {
    json!({
        "version": 1,
        "id": id,
        "type": "pairing.request",
        "payload": {
            "deviceId": device_id,
            "deviceName": device_name,
            "desktopId": "desktop-1",
            "requestNonce": nonce
        }
    })
}

#[test]
fn frames_round_trip_out_of_order_and_ignore_duplicate_chunks() {
    let message = "x".repeat(500);
    let frames = create_frames(&message).unwrap();
    let mut decoded: Vec<BluetoothFrame> = frames
        .iter()
        .map(|frame| serde_json::from_slice(frame).unwrap())
        .collect();
    let duplicate = decoded[1].clone();
    decoded.swap(0, 2);
    decoded.insert(2, duplicate);
    let mut reassembler = FrameReassembler::default();
    let mut result = None;
    for frame in decoded {
        result = reassembler.accept(frame, NOW).unwrap().or(result);
    }
    assert_eq!(result.as_deref(), Some(message.as_str()));
}

#[test]
fn rejects_oversized_and_expires_partial_messages() {
    let mut reassembler = FrameReassembler::default();
    let oversized = BluetoothFrame {
        version: 1,
        message_id: "large".into(),
        sequence: 0,
        is_final: true,
        total_bytes: MAX_MESSAGE_BYTES as i64 + 1,
        payload_base64: String::new(),
    };
    assert_eq!(
        reassembler.accept(oversized, NOW),
        Err("message_too_large".into())
    );

    let partial = BluetoothFrame {
        version: 1,
        message_id: "partial".into(),
        sequence: 0,
        is_final: false,
        total_bytes: 2,
        payload_base64: general_purpose::STANDARD.encode(b"a"),
    };
    assert_eq!(reassembler.accept(partial, NOW).unwrap(), None);
    assert_eq!(reassembler.clear_expired(NOW + PARTIAL_TIMEOUT_MS), 1);
}

#[test]
fn rejects_malformed_chunks_and_bounds_partial_message_floods() {
    let frame =
        |message_id: &str, sequence, is_final, total_bytes, payload: &[u8]| BluetoothFrame {
            version: PROTOCOL_VERSION,
            message_id: message_id.into(),
            sequence,
            is_final,
            total_bytes,
            payload_base64: general_purpose::STANDARD.encode(payload),
        };

    for invalid in [
        frame("empty", 0, false, 1, b""),
        frame("sequence", 2, true, 2, b"a"),
        frame("chunk", 0, true, 161, &[b'a'; FRAME_PAYLOAD_BYTES + 1]),
    ] {
        assert_eq!(
            FrameReassembler::default().accept(invalid, NOW),
            Err("invalid_frame".into())
        );
    }

    let mut aggregate = FrameReassembler::default();
    assert_eq!(
        aggregate.accept(frame("aggregate", 0, false, 2, b"aa"), NOW),
        Ok(None)
    );
    assert_eq!(
        aggregate.accept(frame("aggregate", 1, true, 2, b"b"), NOW),
        Err("invalid_frame".into())
    );

    let oversized_id = "é".repeat(MAX_IDENTIFIER_BYTES / 2 + 1);
    assert_eq!(
        FrameReassembler::default().accept(frame(&oversized_id, 0, true, 1, b"a"), NOW,),
        Err("invalid_frame".into())
    );

    let mut flooded = FrameReassembler::default();
    for index in 0..MAX_PARTIAL_MESSAGES {
        assert_eq!(
            flooded.accept(frame(&format!("partial-{index}"), 0, false, 2, b"a"), NOW,),
            Ok(None)
        );
    }
    assert_eq!(
        flooded.accept(frame("partial-overflow", 0, false, 2, b"a"), NOW),
        Err("invalid_frame".into())
    );
    assert_eq!(
        flooded.accept(frame("partial-0", 1, true, 2, b"b"), NOW),
        Ok(Some("ab".into()))
    );
}

#[test]
fn empty_message_requires_one_final_empty_chunk() {
    let valid = BluetoothFrame {
        version: PROTOCOL_VERSION,
        message_id: "empty".into(),
        sequence: 0,
        is_final: true,
        total_bytes: 0,
        payload_base64: String::new(),
    };
    assert_eq!(
        FrameReassembler::default().accept(valid, NOW),
        Ok(Some(String::new()))
    );
}

#[test]
fn matches_pairing_code_fixture() {
    assert_eq!(
        verification_code("desktop-1", "android-1", "nonce-1"),
        "610717"
    );
}

#[test]
fn matches_android_hmac_and_slash_escaping_fixtures() {
    for (id, text, proof) in [
        (
            "apostrophe-1",
            "'",
            "W0OLnbhllDOCd0Gf_00WLpHRvfidYjHeY69nbcmTFYA",
        ),
        (
            "slash-1",
            "</",
            "70F2Z7SU6ur1gSYzR3Q9t1y5D02z_OuXfst15lQ_3yg",
        ),
        (
            "plain-slash-1",
            "/",
            "ckTEIQrJqsQSd7zoFkIaMzs0wuGvsDeaZwGvFnnU50E",
        ),
    ] {
        let command = json!({
            "version": 1,
            "id": id,
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": "keyboard.typeText",
            "payload": { "text": text },
            "auth": proof
        });
        assert!(auth_matches(&command, TOKEN, proof), "fixture {id}");
    }
}

#[test]
fn validates_utf16_length_and_controls() {
    assert!(is_safe_typed_text("hello\nworld\t"));
    assert!(is_safe_typed_text(&"😀".repeat(1_000)));
    assert!(!is_safe_typed_text(&"😀".repeat(1_001)));
    assert!(!is_safe_typed_text("hello\0world"));
    assert!(!is_safe_typed_text("bad\u{0085}"));
}

#[test]
fn expired_licence_blocks_input_but_preserves_authenticated_connection() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.set_paired_token("remote-1".into(), TOKEN.into());
    engine.set_access_deadline(Some(NOW / 1000));
    for (id, kind, payload, expected) in [
        (
            "locked-text",
            "keyboard.typeText",
            json!({"text":"private"}),
            false,
        ),
        ("locked-mouse", "mouse.move", json!({"dx":1,"dy":1}), false),
        (
            "locked-drag",
            "mouse.dragStart",
            json!({"button":"left"}),
            false,
        ),
        (
            "locked-modifier",
            "keyboard.modifierDown",
            json!({"key":"ctrl"}),
            false,
        ),
        ("locked-ping", "connection.ping", json!({}), true),
    ] {
        let mut command = json!({"version":1,"id":id,"deviceId":"remote-1","timestamp":NOW,"type":kind,"payload":payload});
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let event = engine.process_message(&command.to_string(), NOW).unwrap();
        if expected {
            assert!(matches!(event, EngineEvent::AuthenticatedConnection(_)));
        } else {
            assert!(
                matches!(event,EngineEvent::Response(response) if response.contains("invalid_state") && !response.contains("private"))
            );
        }
    }
}

#[test]
fn licence_expiry_allows_authenticated_stop_and_disconnect_after_input_started() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.set_paired_token("remote-1".into(), TOKEN.into());
    let mut movement = json!({"version":1,"id":"before-expiry","deviceId":"remote-1","timestamp":NOW,"type":"mouse.move","payload":{"dx":1,"dy":1}});
    sign(&mut movement, TOKEN, SlashEscaping::AndroidHtmlSafe);
    assert!(matches!(
        engine.process_message(&movement.to_string(), NOW).unwrap(),
        EngineEvent::MouseMove(_)
    ));
    engine.set_access_deadline(Some(NOW / 1000));
    for (id, kind, payload) in [
        ("stop-expired", "mouse.repeat.stop", json!({})),
        ("drag-expired", "mouse.dragEnd", json!({})),
        (
            "modifier-expired",
            "keyboard.modifierUp",
            json!({"key":"ctrl"}),
        ),
        ("disconnect-expired", "connection.disconnecting", json!({})),
    ] {
        let mut command = json!({"version":1,"id":id,"deviceId":"remote-1","timestamp":NOW,"type":kind,"payload":payload});
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        assert!(matches!(
            engine.process_message(&command.to_string(), NOW).unwrap(),
            EngineEvent::Desktop(_)
        ));
    }
}

#[test]
fn authenticated_ping_emits_connection_events_for_legacy_and_named_payloads() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.set_paired_token("remote-1".into(), TOKEN.into());

    let mut legacy = json!({
        "version": 1,
        "id": "ping-legacy",
        "deviceId": "remote-1",
        "timestamp": NOW,
        "type": "connection.ping",
        "payload": {}
    });
    sign(&mut legacy, TOKEN, SlashEscaping::AndroidHtmlSafe);
    let EngineEvent::AuthenticatedConnection(legacy_connection) =
        engine.process_message(&legacy.to_string(), NOW).unwrap()
    else {
        panic!("expected an authenticated connection");
    };
    assert_eq!(legacy_connection.device_id, "remote-1");
    assert_eq!(legacy_connection.device_name, None);
    assert_eq!(legacy_connection.connected_at, NOW);
    assert_eq!(legacy_connection.received_order, 1);
    assert!(engine
        .complete_authenticated_connection(&legacy_connection, false)
        .contains("\"type\":\"ack\""));

    let mut named = json!({
        "version": 1,
        "id": "ping-named",
        "deviceId": "remote-1",
        "timestamp": NOW + 1,
        "type": "connection.ping",
        "payload": { "deviceName": "  Kitchen Remote 📱  " }
    });
    sign(&mut named, TOKEN, SlashEscaping::AndroidHtmlSafe);
    let EngineEvent::AuthenticatedConnection(connection) =
        engine.process_message(&named.to_string(), NOW + 1).unwrap()
    else {
        panic!("expected an authenticated connection");
    };
    assert_eq!(connection.device_id, "remote-1");
    assert_eq!(connection.device_name.as_deref(), Some("Kitchen Remote 📱"));
    assert_eq!(connection.connected_at, NOW + 1);
    assert_eq!(connection.received_order, 2);
    assert!(engine
        .complete_authenticated_connection(&connection, true)
        .contains("\"type\":\"ack\""));
    assert!(engine
        .complete_authenticated_connection(&connection, false)
        .contains("name_update_failed"));
}

#[test]
fn remote_names_reject_controls_and_more_than_forty_unicode_characters() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.set_paired_token("remote-1".into(), TOKEN.into());
    for (id, device_name) in [
        ("ping-control", "Phone\nSpoof".to_owned()),
        ("ping-long", "📱".repeat(41)),
    ] {
        let mut command = json!({
            "version": 1,
            "id": id,
            "deviceId": "remote-1",
            "timestamp": NOW,
            "type": "connection.ping",
            "payload": { "deviceName": device_name }
        });
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let EngineEvent::Response(response) =
            engine.process_message(&command.to_string(), NOW).unwrap()
        else {
            panic!("expected an invalid payload response");
        };
        assert!(response.contains("invalid_payload"));
    }

    let invalid_pairing = pairing_request("pair-invalid", "remote-2", "Phone\nSpoof", "nonce");
    let EngineEvent::Response(response) = engine
        .process_message(&invalid_pairing.to_string(), NOW)
        .unwrap()
    else {
        panic!("expected an invalid pairing response");
    };
    assert!(response.contains("invalid_payload"));
    assert!(engine.pending_pairings().is_empty());
}

#[test]
fn pairing_approval_is_memory_only_and_expires() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    let request = pairing_request("pair-1", "android-1", "Pixel", "nonce-1");
    let event = engine.process_message(&request.to_string(), NOW).unwrap();
    assert!(matches!(
        event,
        EngineEvent::PendingPairing {
            replaced_response: None,
            ..
        }
    ));
    assert_eq!(
        engine.pairing_intent("pair-1", NOW + 1),
        Some(PairingIntent {
            desktop_id: "desktop-1".into(),
            device_id: "android-1".into(),
            nonce: "nonce-1".into(),
        })
    );
    assert_eq!(engine.pairing_intent("pair-unknown", NOW + 1), None);
    assert_eq!(
        engine.pairing_intent("pair-1", NOW + PAIRING_TIMEOUT_MS),
        None
    );
    assert!(!serde_json::to_string(&engine.pending_pairings())
        .unwrap()
        .contains("nonce-1"));
    let response = engine.approve_pairing("pair-1", NOW + 1).unwrap();
    assert_eq!(engine.pairing_intent("pair-1", NOW + 1), None);
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["type"], "pairing.complete");
    assert_eq!(response["payload"]["token"].as_str().unwrap().len(), 43);

    let mut expired = ProtocolEngine::new("desktop-1".into());
    expired.process_message(&request.to_string(), NOW).unwrap();
    assert!(expired
        .approve_pairing("pair-1", NOW + PAIRING_TIMEOUT_MS)
        .is_err());
    assert!(expired.pending_pairings().is_empty());
}

#[test]
fn pairing_requests_are_newest_first_and_actions_are_targeted() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine
        .process_message(
            &pairing_request("pair-1", "android-1", "Pixel", "nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    engine
        .process_message(
            &pairing_request("pair-2", "android-2", "Galaxy", "nonce-2").to_string(),
            NOW + 1,
        )
        .unwrap();

    let pending = engine.pending_pairings();
    assert_eq!(
        pending
            .iter()
            .map(|request| request.request_id.as_str())
            .collect::<Vec<_>>(),
        ["pair-2", "pair-1"]
    );
    engine.reject_pairing("pair-1").unwrap();
    assert_eq!(engine.pending_pairings()[0].request_id, "pair-2");
    assert!(engine.reject_pairing("pair-1").is_err());
}

#[test]
fn pairing_queue_is_bounded_but_allows_same_device_replacement() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    for index in 0..MAX_PENDING_PAIRINGS {
        let event = engine
            .process_message(
                &pairing_request(
                    &format!("pair-{index}"),
                    &format!("android-{index}"),
                    "Phone",
                    &format!("nonce-{index}"),
                )
                .to_string(),
                NOW + index as i64,
            )
            .unwrap();
        assert!(matches!(event, EngineEvent::PendingPairing { .. }));
    }

    let overflow = engine
        .process_message(
            &pairing_request("pair-overflow", "android-new", "Phone", "nonce-new").to_string(),
            NOW + 20,
        )
        .unwrap();
    let EngineEvent::Response(overflow) = overflow else {
        panic!("expected pairing overflow response");
    };
    assert!(overflow.contains("invalid_payload"));
    assert_eq!(engine.pending_pairings().len(), MAX_PENDING_PAIRINGS);

    let replacement = engine
        .process_message(
            &pairing_request("pair-replacement", "android-0", "Phone", "nonce-new").to_string(),
            NOW + 21,
        )
        .unwrap();
    assert!(matches!(
        replacement,
        EngineEvent::PendingPairing {
            replaced_response: Some(_),
            ..
        }
    ));
    assert_eq!(engine.pending_pairings().len(), MAX_PENDING_PAIRINGS);
}

#[test]
fn pairing_identifiers_are_limited_by_utf8_bytes() {
    let oversized = "é".repeat(MAX_IDENTIFIER_BYTES / 2 + 1);
    for request in [
        pairing_request(&oversized, "android-1", "Phone", "nonce-1"),
        pairing_request("pair-1", &oversized, "Phone", "nonce-1"),
        pairing_request("pair-1", "android-1", "Phone", &oversized),
        {
            let mut request = pairing_request("pair-1", "android-1", "Phone", "nonce-1");
            request["payload"]["desktopId"] = json!(oversized);
            request
        },
    ] {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        let EngineEvent::Response(response) =
            engine.process_message(&request.to_string(), NOW).unwrap()
        else {
            panic!("expected invalid identifier response");
        };
        assert!(response.contains("invalid_payload"));
        assert!(engine.pending_pairings().is_empty());
    }
}

#[test]
fn pairing_cancellation_is_targeted_and_makes_expiry_harmless() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine
        .process_message(
            &pairing_request("pair-1", "android-1", "Pixel", "nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    engine
        .process_message(
            &pairing_request("pair-2", "android-2", "Galaxy", "nonce-2").to_string(),
            NOW + 1,
        )
        .unwrap();

    assert!(engine.cancel_pairing("pair-1"));
    assert!(!engine.cancel_pairing("pair-1"));
    assert_eq!(engine.pending_pairings()[0].request_id, "pair-2");
    assert_eq!(
        engine.expire_pairing("pair-1", NOW + PAIRING_TIMEOUT_MS),
        None
    );
    assert_eq!(engine.cancel_all_pairings(), 1);
    assert_eq!(engine.cancel_all_pairings(), 0);
    assert!(engine.pending_pairings().is_empty());
}

#[test]
fn transport_reset_clears_partial_frames_and_pairings_but_preserves_security_state() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.set_paired_token("android-1".into(), "token-1".into());
    let partial = BluetoothFrame {
        version: PROTOCOL_VERSION,
        message_id: "partial".into(),
        sequence: 0,
        is_final: false,
        total_bytes: 2,
        payload_base64: general_purpose::STANDARD.encode(b"a"),
    };
    assert_eq!(
        engine.receive_frame(&serde_json::to_vec(&partial).unwrap(), NOW),
        Ok(None)
    );
    engine
        .process_message(
            &pairing_request("pair-1", "android-2", "Phone", "nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    engine
        .replay_cache
        .insert("android-1:request-1".into(), NOW);

    assert_eq!(engine.reset_transport_session(), 1);
    assert!(engine.pending_pairings().is_empty());
    assert_eq!(engine.token_for("android-1"), Some("token-1"));
    assert_eq!(engine.replay_cache.get("android-1:request-1"), Some(&NOW));
    assert!(engine.reassembler.partials.is_empty());
}

#[test]
fn newer_request_from_same_device_replaces_only_that_device() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine
        .process_message(
            &pairing_request("pair-1", "android-1", "Pixel", "nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    engine
        .process_message(
            &pairing_request("pair-2", "android-2", "Galaxy", "nonce-2").to_string(),
            NOW + 1,
        )
        .unwrap();
    let event = engine
        .process_message(
            &pairing_request("pair-3", "android-1", "Pixel", "nonce-3").to_string(),
            NOW + 2,
        )
        .unwrap();

    let EngineEvent::PendingPairing {
        request,
        replaced_response: Some(replaced_response),
    } = event
    else {
        panic!("expected a replacement pairing event");
    };
    assert_eq!(request.request_id, "pair-3");
    assert!(replaced_response.contains("pair-1"));
    assert!(replaced_response.contains("pairing_request_expired"));
    assert_eq!(
        engine
            .pending_pairings()
            .iter()
            .map(|request| request.request_id.as_str())
            .collect::<Vec<_>>(),
        ["pair-3", "pair-2"]
    );
    assert_eq!(
        engine.expire_pairing("pair-1", NOW + PAIRING_TIMEOUT_MS),
        None
    );
}

#[test]
fn duplicate_pairing_request_id_does_not_replace_another_device() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine
        .process_message(
            &pairing_request("pair-1", "android-1", "Pixel", "nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    let original = engine.pending_pairings().remove(0);

    let event = engine
        .process_message(
            &pairing_request("pair-1", "android-2", "Galaxy", "nonce-2").to_string(),
            NOW + 1,
        )
        .unwrap();

    let EngineEvent::Response(response) = event else {
        panic!("expected a duplicate request response");
    };
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["error"]["code"], "duplicate_request");

    let pending = engine.pending_pairings();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id, original.request_id);
    assert_eq!(pending[0].device_id, original.device_id);
    assert_eq!(pending[0].device_name, original.device_name);
    assert_eq!(pending[0].verification_code, original.verification_code);
    assert_eq!(pending[0].expires_at, original.expires_at);
}

#[test]
fn pairing_requests_expire_independently_and_summaries_exclude_nonces() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine
        .process_message(
            &pairing_request("pair-1", "android-1", "Pixel", "secret-nonce-1").to_string(),
            NOW,
        )
        .unwrap();
    engine
        .process_message(
            &pairing_request("pair-2", "android-2", "Galaxy", "secret-nonce-2").to_string(),
            NOW + 1,
        )
        .unwrap();

    let summaries = serde_json::to_string(&engine.pending_pairings()).unwrap();
    assert!(!summaries.contains("nonce"));
    let expired = engine
        .expire_pairing("pair-1", NOW + PAIRING_TIMEOUT_MS)
        .unwrap();
    assert!(expired.contains("pairing_request_expired"));
    assert_eq!(engine.pending_pairings()[0].request_id, "pair-2");
    assert_eq!(
        engine.expire_pairing("pair-2", NOW + PAIRING_TIMEOUT_MS),
        None
    );
}

#[test]
fn rejects_expired_and_replayed_authenticated_commands() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.tokens.insert("android-1".into(), TOKEN.into());
    let mut command = json!({
        "version": 1,
        "id": "text-1",
        "deviceId": "android-1",
        "timestamp": NOW,
        "type": "keyboard.typeText",
        "payload": { "text": "Hello" },
        "auth": ""
    });
    sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
    assert!(matches!(
        engine.process_message(&command.to_string(), NOW).unwrap(),
        EngineEvent::Text(_)
    ));
    let duplicate = engine.process_message(&command.to_string(), NOW).unwrap();
    assert!(
        matches!(duplicate, EngineEvent::Response(response) if response.contains("duplicate_request"))
    );

    command["id"] = Value::String("expired".into());
    command["timestamp"] = Value::from(NOW - COMMAND_TIMESTAMP_TOLERANCE_MS - 1);
    sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
    let expired = engine.process_message(&command.to_string(), NOW).unwrap();
    assert!(
        matches!(expired, EngineEvent::Response(response) if response.contains("expired_timestamp"))
    );
}

#[test]
fn rejects_replay_of_future_stamped_command_after_receive_window() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.tokens.insert("android-1".into(), TOKEN.into());
    let skew = 30_000;
    let mut command = json!({
        "version": 1,
        "id": "text-1",
        "deviceId": "android-1",
        "timestamp": NOW + skew,
        "type": "keyboard.typeText",
        "payload": { "text": "Hello" },
        "auth": ""
    });
    sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
    assert!(matches!(
        engine.process_message(&command.to_string(), NOW).unwrap(),
        EngineEvent::Text(_)
    ));

    // Just after the receive-time window closes the command is still
    // within tolerance of its own timestamp, so it must stay cached.
    let later = NOW + COMMAND_TIMESTAMP_TOLERANCE_MS + 1;
    let replayed = engine.process_message(&command.to_string(), later).unwrap();
    assert!(
        matches!(replayed, EngineEvent::Response(response) if response.contains("duplicate_request"))
    );

    // The last millisecond the timestamp check accepts is still cached.
    let edge = NOW + skew + COMMAND_TIMESTAMP_TOLERANCE_MS;
    let replayed_at_edge = engine.process_message(&command.to_string(), edge).unwrap();
    assert!(
        matches!(replayed_at_edge, EngineEvent::Response(response) if response.contains("duplicate_request"))
    );

    // Once the command's own window has closed it is rejected as expired.
    let expired_at = NOW + skew + COMMAND_TIMESTAMP_TOLERANCE_MS + 1;
    let expired = engine
        .process_message(&command.to_string(), expired_at)
        .unwrap();
    assert!(
        matches!(expired, EngineEvent::Response(response) if response.contains("expired_timestamp"))
    );
}

#[test]
fn no_ack_text_suppresses_only_the_success_response() {
    let engine = ProtocolEngine::new("desktop-1".into());
    let command = TextCommand {
        id: "text-1".into(),
        text: "Hello".into(),
        response_mode: ResponseMode::None,
    };
    assert_eq!(engine.complete_text_command(&command, Ok(())), None);
    assert!(engine
        .complete_text_command(&command, Err("accessibility_required"))
        .unwrap()
        .contains("input_failed"));
}

#[test]
fn authenticated_mouse_move_supports_ack_and_no_ack_modes() {
    for (id, response_mode, expected_mode) in [
        ("move-ack", None, ResponseMode::Ack),
        ("move-none", Some("none"), ResponseMode::None),
    ] {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut command = json!({
            "version": 1,
            "id": id,
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": "mouse.move",
            "payload": { "dx": 12, "dy": -6 },
            "auth": ""
        });
        if let Some(response_mode) = response_mode {
            command["responseMode"] = Value::String(response_mode.into());
        }
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let event = engine.process_message(&command.to_string(), NOW).unwrap();
        let EngineEvent::MouseMove(command) = event else {
            panic!("expected a mouse move command");
        };
        assert_eq!((command.dx, command.dy), (12.0, -6.0));
        assert_eq!(command.response_mode, expected_mode);
        let response = engine.complete_mouse_move_command(&command, Ok(()));
        assert_eq!(response.is_none(), expected_mode == ResponseMode::None);
    }
}

#[test]
fn mouse_move_validates_boundaries_and_reports_no_ack_failures() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.tokens.insert("android-1".into(), TOKEN.into());
    let mut boundary = json!({
        "version": 1,
        "id": "move-boundary",
        "deviceId": "android-1",
        "timestamp": NOW,
        "type": "mouse.move",
        "payload": { "dx": -500, "dy": 500 },
        "responseMode": "none",
        "auth": ""
    });
    sign(&mut boundary, TOKEN, SlashEscaping::AndroidHtmlSafe);
    let EngineEvent::MouseMove(command) =
        engine.process_message(&boundary.to_string(), NOW).unwrap()
    else {
        panic!("expected a boundary mouse move command");
    };
    assert!(engine
        .complete_mouse_move_command(&command, Err("accessibility_required"))
        .unwrap()
        .contains("input_failed"));

    for (id, payload) in [
        ("move-too-large", json!({ "dx": 501, "dy": 0 })),
        ("move-malformed", json!({ "dx": "right", "dy": 0 })),
    ] {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut invalid = json!({
            "version": 1,
            "id": id,
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": "mouse.move",
            "payload": payload,
            "auth": ""
        });
        sign(&mut invalid, TOKEN, SlashEscaping::AndroidHtmlSafe);
        assert!(matches!(
            engine.process_message(&invalid.to_string(), NOW).unwrap(),
            EngineEvent::Response(response) if response.contains("invalid_payload")
        ));
    }
}

#[test]
fn authenticated_mouse_clicks_support_buttons_counts_and_no_ack() {
    for (id, command_type, payload, expected_button, expected_count) in [
        (
            "click-left",
            "mouse.click",
            json!({ "button": "left" }),
            MouseButton::Left,
            1,
        ),
        (
            "double-middle",
            "mouse.doubleClick",
            json!({ "button": "middle" }),
            MouseButton::Middle,
            2,
        ),
        (
            "right-click",
            "mouse.rightClick",
            json!({}),
            MouseButton::Right,
            1,
        ),
    ] {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut command = json!({
            "version": 1,
            "id": id,
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": command_type,
            "payload": payload,
            "responseMode": "none",
            "auth": ""
        });
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let EngineEvent::MouseClick(command) =
            engine.process_message(&command.to_string(), NOW).unwrap()
        else {
            panic!("expected a mouse click command");
        };
        assert_eq!(command.button, expected_button);
        assert_eq!(command.click_count, expected_count);
        assert_eq!(engine.complete_mouse_click_command(&command, Ok(())), None);
        assert!(engine
            .complete_mouse_click_command(&command, Err("accessibility_required"))
            .unwrap()
            .contains("input_failed"));
    }
}

#[test]
fn mouse_clicks_reject_invalid_button_payloads() {
    for (id, command_type, payload) in [
        (
            "click-invalid",
            "mouse.click",
            json!({ "button": "primary" }),
        ),
        (
            "right-invalid",
            "mouse.rightClick",
            json!({ "button": "right" }),
        ),
    ] {
        let mut engine = ProtocolEngine::new("desktop-1".into());
        engine.tokens.insert("android-1".into(), TOKEN.into());
        let mut command = json!({
            "version": 1,
            "id": id,
            "deviceId": "android-1",
            "timestamp": NOW,
            "type": command_type,
            "payload": payload,
            "auth": ""
        });
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        assert!(matches!(
            engine.process_message(&command.to_string(), NOW).unwrap(),
            EngineEvent::Response(response) if response.contains("invalid_payload")
        ));
    }
}

#[test]
fn mouse_repeat_accepts_only_exact_bounded_nested_pointer_commands() {
    assert!(valid_desktop_payload(
        "mouse.repeat.start",
        &json!({"command":{"type":"mouse.move","payload":{"dx":500,"dy":-500}}})
    ));
    assert!(valid_desktop_payload(
        "mouse.repeat.start",
        &json!({"command":{"type":"mouse.scroll","payload":{"dx":50,"dy":-50}}})
    ));
    for payload in [
        json!({"command":{"type":"mouse.click","payload":{}}}),
        json!({"command":{"type":"mouse.move","payload":{"dx":501,"dy":0}}}),
        json!({"command":{"type":"mouse.scroll","payload":{"dx":0,"dy":51}}}),
        json!({"command":{"type":"mouse.move","payload":{"dx":1,"dy":1,"extra":true}}}),
    ] {
        assert!(!valid_desktop_payload("mouse.repeat.start", &payload));
    }
    assert!(valid_desktop_payload("mouse.repeat.stop", &json!({})));
    assert!(!valid_desktop_payload(
        "mouse.repeat.stop",
        &json!({"deviceId":"extra"})
    ));
}

#[test]
fn repeat_start_accepts_every_repeatable_key_and_rejects_the_rest() {
    for key in RepeatKey::ALL {
        assert!(
            valid_desktop_payload(
                "mouse.repeat.start",
                &json!({"command":{"type":"keyboard.key","payload":{"key":key.protocol_name()}}})
            ),
            "{} should be repeatable",
            key.protocol_name()
        );
    }
    for payload in [
        // Printable characters and Space would flood text, modifiers belong
        // to keyboard.modifierDown, and Enter would re-submit on every tick.
        json!({"command":{"type":"keyboard.key","payload":{"key":"a"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"A"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Space"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Enter"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Escape"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Ctrl"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Shift"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"Home"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":"F1"}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":""}}}),
        // Shape violations.
        json!({"command":{"type":"keyboard.key","payload":{"key":"Tab","extra":true}}}),
        json!({"command":{"type":"keyboard.key","payload":{}}}),
        json!({"command":{"type":"keyboard.key","payload":{"key":1}}}),
        json!({"command":{"type":"keyboard.shortcut","payload":{"keys":["Tab"]}}}),
    ] {
        assert!(
            !valid_desktop_payload("mouse.repeat.start", &payload),
            "{payload} should be rejected"
        );
    }
}

#[test]
fn repeat_start_still_accepts_the_v1_pointer_payloads_byte_for_byte() {
    // Pins the exact shapes shipped clients send today. Widening the nested
    // envelope for keyboard.key must not move these acceptance boundaries.
    for payload in [
        json!({"command":{"type":"mouse.move","payload":{"dx":0,"dy":0}}}),
        json!({"command":{"type":"mouse.move","payload":{"dx":-64,"dy":128}}}),
        json!({"command":{"type":"mouse.move","payload":{"dx":500,"dy":-500}}}),
        json!({"command":{"type":"mouse.scroll","payload":{"dx":0,"dy":5}}}),
        json!({"command":{"type":"mouse.scroll","payload":{"dx":-50,"dy":50}}}),
    ] {
        assert!(
            valid_desktop_payload("mouse.repeat.start", &payload),
            "{payload} is a shipped v1 payload"
        );
    }
    for payload in [
        json!({"command":{"type":"mouse.move","payload":{"dx":501,"dy":0}}}),
        json!({"command":{"type":"mouse.scroll","payload":{"dx":0,"dy":51}}}),
        json!({"command":{"type":"mouse.move","payload":{"dx":1}}}),
        json!({"command":{"type":"mouse.scroll","payload":{"dx":1,"dy":1,"extra":0}}}),
    ] {
        assert!(
            !valid_desktop_payload("mouse.repeat.start", &payload),
            "{payload} was rejected before this change"
        );
    }
}

#[test]
fn pointer_profile_advertises_key_repeat_without_disturbing_mouse_repeat() {
    let profile = PointerProfile {
        display_id: "display:0:0:1920:1080:1".into(),
        scale_factor: 1.0,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        small_delta: 49,
        medium_delta: 130,
        large_delta: 281,
        display_navigation_supported: true,
        display_count: 2,
    };
    let settings = AppSettings::default();
    let response: Value =
        serde_json::from_str(&pointer_profile_response("profile-1", &profile, &settings)).unwrap();
    let capabilities = &response["payload"]["capabilities"];

    let key_repeat = &capabilities["keyRepeat"];
    assert_eq!(key_repeat["supported"], true);
    assert_eq!(key_repeat["enabled"], settings.key_repeat_enabled);
    assert_eq!(key_repeat["intervalMs"], settings.key_repeat_interval_ms);
    assert_eq!(
        key_repeat["initialDelayMs"],
        settings.key_repeat_initial_delay_ms
    );
    assert_eq!(key_repeat["minIntervalMs"], 100);
    assert_eq!(key_repeat["maxIntervalMs"], 1000);
    assert_eq!(
        key_repeat["repeatableKeys"],
        json!([
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Tab",
            "Backspace",
            "Delete",
            "PageUp",
            "PageDown"
        ])
    );

    // An old client reads only these; adding keyRepeat must not move them.
    assert_eq!(
        capabilities["mouseRepeat"],
        json!({
            "supported": true,
            "enabled": settings.mouse_repeat_enabled,
            "intervalMs": settings.move_repeat_interval_ms,
            "moveIntervalMs": settings.move_repeat_interval_ms,
            "scrollIntervalMs": settings.scroll_repeat_interval_ms,
            "minIntervalMs": 100,
            "maxIntervalMs": 2000,
            "accelerationDurationMs": settings.mouse_repeat_acceleration_duration_ms,
            "accelerationDurationOptionsMs": [0, 500, 1000, 2000],
            "accelerationInitialScalePercent": 25
        })
    );

    // The current Remote reads baseMoveDelta; older clients may still use
    // recommendedDeltas. Keep their profile shape while matching Mouse scanning.
    for (speed, expected_effective) in [(5, 1), (100, 12), (225, 27), (1350, 162)] {
        let mut settings = settings.clone();
        settings.pointer_scale_percent = speed;
        let response: Value =
            serde_json::from_str(&pointer_profile_response("profile-1", &profile, &settings))
                .unwrap();
        assert_eq!(
            response["payload"]["capabilities"]["pointerSpeed"]["baseMoveDelta"],
            12
        );
        assert_eq!(
            response["payload"]["capabilities"]["pointerSpeed"]["effectiveMoveDelta"],
            expected_effective
        );
        assert_eq!(response["payload"]["recommendedDeltas"]["medium"], 130);
    }
}

#[test]
fn repeat_command_names_stay_out_of_the_parallel_command_lists() {
    // Key repeat rides the existing repeat envelope precisely so these
    // hand-maintained lists need no new entries. Pinning them here makes an
    // accidental future divergence fail loudly.
    let profile = PointerProfile {
        display_id: "display:0:0:1920:1080:1".into(),
        scale_factor: 1.0,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        small_delta: 49,
        medium_delta: 130,
        large_delta: 281,
        display_navigation_supported: true,
        display_count: 2,
    };
    let response: Value = serde_json::from_str(&pointer_profile_response(
        "profile-1",
        &profile,
        &AppSettings::default(),
    ))
    .unwrap();
    let capabilities = &response["payload"]["capabilities"];
    for list in ["supportedCommands", "noAckCommands"] {
        let commands = capabilities[list].as_array().unwrap();
        assert!(commands.contains(&json!("mouse.repeat.start")), "{list}");
        assert!(commands.contains(&json!("mouse.repeat.stop")), "{list}");
        for absent in [
            "keyboard.repeat.start",
            "keyboard.repeat.stop",
            "input.repeat.start",
            "input.repeat.stop",
        ] {
            assert!(!commands.contains(&json!(absent)), "{list} gained {absent}");
        }
    }
    for absent in ["keyboard.repeat.start", "input.repeat.start"] {
        assert!(!is_desktop_command(absent), "{absent}");
    }
    assert!(is_desktop_command("mouse.repeat.start"));
    assert!(is_desktop_command("mouse.repeat.stop"));
}

#[test]
fn no_ack_mouse_repeat_commands_are_accepted_without_a_response() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.tokens.insert("android-1".into(), TOKEN.into());
    for (index, command_type) in ["mouse.repeat.start", "mouse.repeat.stop"]
        .into_iter()
        .enumerate()
    {
        assert!(accepts_no_ack(command_type));
        let payload = if command_type == "mouse.repeat.start" {
            json!({ "command": { "type": "mouse.move", "payload": { "dx": 1, "dy": 0 } } })
        } else {
            json!({})
        };
        let mut command = json!({
            "version": 1,
            "id": format!("repeat-{index}"),
            "deviceId": "android-1",
            "timestamp": NOW + index as i64,
            "type": command_type,
            "payload": payload,
            "responseMode": "none",
            "auth": ""
        });
        sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
        let event = engine
            .process_message(&command.to_string(), NOW + index as i64)
            .unwrap();
        let EngineEvent::Desktop(command) = event else {
            panic!("{command_type} with responseMode none should be accepted, got {event:?}");
        };
        assert_eq!(command.command_type, command_type);
        assert_eq!(command.response_mode, ResponseMode::None);
        assert_eq!(engine.complete_desktop_command(&command, Ok(())), None);
        assert!(engine
            .complete_desktop_command(&command, Err("input_failed"))
            .unwrap()
            .contains("input_failed"));
    }

    let mut rejected = json!({
        "version": 1,
        "id": "profile-none",
        "deviceId": "android-1",
        "timestamp": NOW + 10,
        "type": "pointer.profile",
        "payload": {},
        "responseMode": "none",
        "auth": ""
    });
    sign(&mut rejected, TOKEN, SlashEscaping::AndroidHtmlSafe);
    assert!(matches!(
        engine.process_message(&rejected.to_string(), NOW + 10).unwrap(),
        EngineEvent::Response(response) if response.contains("invalid_payload")
    ));
}

#[test]
fn pointer_profile_advertises_the_full_desktop_command_set() {
    let profile = PointerProfile {
        display_id: "display:0:0:1920:1080:1".into(),
        scale_factor: 1.0,
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
        small_delta: 49,
        medium_delta: 130,
        large_delta: 281,
        display_navigation_supported: true,
        display_count: 2,
    };
    let response: Value = serde_json::from_str(&pointer_profile_response(
        "profile-1",
        &profile,
        &AppSettings::default(),
    ))
    .unwrap();
    assert_eq!(response["payload"]["maxDelta"], 500);
    assert_eq!(response["payload"]["recommendedDeltas"]["medium"], 130);
    assert_eq!(response["payload"]["capabilities"]["noAckMouseMove"], true);
    assert_eq!(
        response["payload"]["capabilities"]["pointerSpeed"]["maxScalePercent"],
        1350
    );
    assert_eq!(
        response["payload"]["capabilities"]["mouseRepeat"]["supported"],
        true
    );
    assert_eq!(
        response["payload"]["capabilities"]["mouseRepeat"]["accelerationInitialScalePercent"],
        25
    );
    let commands = response["payload"]["capabilities"]["supportedCommands"]
        .as_array()
        .unwrap();
    for command in [
        "mouse.move",
        "mouse.scroll",
        "keyboard.shortcut",
        "keyboard.typeText",
        "media.control",
        "window.control",
        "switch.session.start",
        "pointer.display.move",
    ] {
        assert!(commands.contains(&json!(command)), "missing {command}");
    }
    assert_eq!(
        response["payload"]["capabilities"]["displayNavigation"]["supported"],
        true
    );
    assert_eq!(
        response["payload"]["capabilities"]["displayNavigation"]["displayCount"],
        2
    );

    let unsupported = PointerProfile {
        display_navigation_supported: false,
        ..profile
    };
    let response: Value = serde_json::from_str(&pointer_profile_response(
        "profile-unsupported",
        &unsupported,
        &AppSettings::default(),
    ))
    .unwrap();
    assert_eq!(
        response["payload"]["capabilities"]["displayNavigation"]["supported"],
        false
    );
    assert!(!response["payload"]["capabilities"]["supportedCommands"]
        .as_array()
        .unwrap()
        .contains(&json!("pointer.display.move")));
}

#[test]
fn display_navigation_accepts_only_one_bounded_direction() {
    for direction in ["left", "right", "up", "down"] {
        assert!(valid_desktop_payload(
            "pointer.display.move",
            &json!({"direction": direction})
        ));
    }
    for payload in [
        json!({}),
        json!({"direction": "diagonal"}),
        json!({"direction": "left", "extra": true}),
        json!({"direction": 1}),
    ] {
        assert!(!valid_desktop_payload("pointer.display.move", &payload));
    }
}

#[test]
fn display_navigation_executes_only_after_authentication() {
    let mut engine = ProtocolEngine::new("desktop-1".into());
    engine.tokens.insert("android-1".into(), TOKEN.into());
    let mut command = json!({
        "version": 1,
        "id": "display-auth",
        "deviceId": "android-1",
        "timestamp": NOW,
        "type": "pointer.display.move",
        "payload": { "direction": "right" },
        "auth": ""
    });
    let mut rejected = command.clone();
    rejected["id"] = Value::String("display-rejected".into());
    rejected["auth"] = Value::String("bad-proof".into());
    assert!(matches!(
        engine.process_message(&rejected.to_string(), NOW).unwrap(),
        EngineEvent::Response(response) if response.contains("invalid_auth")
    ));

    sign(&mut command, TOKEN, SlashEscaping::AndroidHtmlSafe);
    let event = engine.process_message(&command.to_string(), NOW).unwrap();
    let EngineEvent::Desktop(command) = event else {
        panic!("expected a desktop command");
    };
    assert_eq!(command.command_type, "pointer.display.move");
    assert_eq!(command.payload["direction"], "right");
}

#[test]
fn desktop_completion_can_preserve_structured_errors() {
    let engine = ProtocolEngine::new("desktop".into());
    let command = DesktopCommand {
        id: "display-1".into(),
        device_id: "device".into(),
        command_type: "pointer.display.move".into(),
        payload: json!({"direction": "right"}),
        response_mode: ResponseMode::Ack,
    };
    let response: Value = serde_json::from_str(
        &engine
            .complete_desktop_command_with_error(
                &command,
                Err(("no_display_in_direction", "No monitor to the right.")),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["error"]["code"], "no_display_in_direction");
    assert_eq!(response["error"]["message"], "No monitor to the right.");
}

#[test]
fn notification_frames_fit_the_negotiated_limit_and_round_trip() {
    let message = json!({
        "version": 1,
        "id": "android-00000000-0000-0000-0000-000000000000",
        "type": "ack",
        "ok": true,
        "error": Value::Null,
        "padding": "x".repeat(400)
    })
    .to_string();
    let frames = create_notification_frames(&message, 182).unwrap();
    assert!(frames.len() > 1);
    assert!(frames.iter().all(|frame| frame.len() <= 182));

    let mut reassembler = FrameReassembler::default();
    let mut result = None;
    for frame in frames {
        let frame = serde_json::from_slice(&frame).unwrap();
        result = reassembler.accept(frame, NOW).unwrap().or(result);
    }
    assert_eq!(result.as_deref(), Some(message.as_str()));
    assert!(create_notification_frames(&message, 64).is_err());
}

#[test]
fn outbound_queue_preserves_backpressured_frame() {
    let mut queue = OutboundQueue::default();
    queue.push_message(&"x".repeat(400), 10).unwrap();
    let initial = queue.len();
    let mut attempts = 0;
    queue
        .flush(|_, _| {
            attempts += 1;
            Ok(false)
        })
        .unwrap();
    assert_eq!(attempts, 1);
    assert_eq!(queue.len(), initial);
    queue.flush(|_, _| Ok(true)).unwrap();
    assert_eq!(queue.len(), 0);
}

#[test]
fn outbound_queue_keeps_each_frame_target() {
    let mut queue = OutboundQueue::default();
    queue
        .push_notification_message(
            "broadcast",
            10,
            FRAME_PAYLOAD_BYTES * 2,
            NotificationTarget::AllSubscribers,
        )
        .unwrap();
    queue
        .push_notification_message(
            &"x".repeat(400),
            10,
            FRAME_PAYLOAD_BYTES * 2,
            NotificationTarget::Central("central-a".into()),
        )
        .unwrap();
    let mut targets = Vec::new();
    queue
        .flush(|_, target| {
            targets.push(target.clone());
            Ok(true)
        })
        .unwrap();
    assert_eq!(targets[0], NotificationTarget::AllSubscribers);
    assert!(targets.len() > 2);
    assert!(targets[1..]
        .iter()
        .all(|target| *target == NotificationTarget::Central("central-a".into())));
}

#[test]
fn pairing_central_registry_cancels_only_the_disconnected_central() {
    let mut registry = PairingCentralRegistry::default();
    registry.associate("pair-a-old".into(), "central-a".into());
    registry.associate("pair-a".into(), "central-a".into());
    registry.associate("pair-b".into(), "central-b".into());

    registry.retain_pending(&HashSet::from(["pair-a".to_string(), "pair-b".to_string()]));
    assert_eq!(registry.central_for("pair-a"), Some("central-a"));
    assert_eq!(registry.central_for("pair-a-old"), None);
    let mut cancelled = registry.take_for_central("central-a");
    cancelled.sort();

    assert_eq!(cancelled, ["pair-a"]);
    assert!(registry.take_for_central("unknown").is_empty());
    assert_eq!(registry.take_for_central("central-b"), ["pair-b"]);
}

#[test]
fn pairing_central_registry_takes_requests_from_missing_centrals() {
    let mut registry = PairingCentralRegistry::default();
    registry.associate("pair-a".into(), "central-a".into());
    registry.associate("pair-b".into(), "central-b".into());

    let connected = HashSet::from(["central-b".to_string()]);
    assert_eq!(registry.take_disconnected(&connected), ["pair-a"]);
    assert_eq!(registry.central_for("pair-a"), None);
    assert_eq!(registry.central_for("pair-b"), Some("central-b"));
    assert!(registry.take_disconnected(&connected).is_empty());
    assert_eq!(registry.remove("pair-b"), Some("central-b".into()));
}

fn sign(command: &mut Value, token: &str, mode: SlashEscaping) {
    let canonical = canonical_command(command, mode).unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(token.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    command["auth"] =
        Value::String(general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()));
}

/// Fails when a desktop command is added to `COMMANDS` without a sample here.
/// The sample must pass `valid_desktop_payload`, which rejects commands it does
/// not list, so the command also needs its own validator arm.
#[test]
fn every_desktop_command_has_a_payload_rule() {
    let session = "8f14e45f-ceea-467a-9b36-4a3c1f8e2b10";
    let samples = [
        ("mouse.scroll", json!({"dx":0,"dy":3})),
        ("mouse.dragStart", json!({"button":"left"})),
        ("mouse.dragEnd", json!({})),
        (
            "mouse.repeat.start",
            json!({"command":{"type":"mouse.move","payload":{"dx":5,"dy":0}}}),
        ),
        ("mouse.repeat.stop", json!({})),
        ("pointer.speed.set", json!({"scalePercent":100})),
        ("pointer.display.move", json!({"direction":"left"})),
        ("keyboard.key", json!({"key":"a"})),
        ("keyboard.modifierDown", json!({"key":"Shift"})),
        ("keyboard.modifierUp", json!({"key":"Shift"})),
        ("keyboard.shortcut", json!({"keys":["Ctrl","C"]})),
        ("keyboard.textStream.open", json!({"streamId":"s1"})),
        (
            "keyboard.textStream.char",
            json!({"streamId":"s1","seq":0,"text":"a"}),
        ),
        (
            "keyboard.textStream.chunk",
            json!({"streamId":"s1","seq":1,"text":"hi"}),
        ),
        (
            "keyboard.textStream.key",
            json!({"streamId":"s1","seq":2,"key":"Enter"}),
        ),
        (
            "keyboard.textStream.close",
            json!({"streamId":"s1","expectedCount":3}),
        ),
        ("media.control", json!({"action":"playPause"})),
        ("window.control", json!({"action":"minimize"})),
        ("grid.switch.set", json!({"switchId":1,"state":"down"})),
        (
            "grid.switch.sync",
            json!({"sessionId":session,"sequence":1,"pressedSwitchIds":[1]}),
        ),
        ("switch.profile.list", json!({})),
        (
            "switch.session.start",
            json!({"sessionId":session,"profileId":"p","profileVersion":1,"switchCount":1}),
        ),
        (
            "switch.edge",
            json!({"sessionId":session,"sequence":1,"switchId":1,"state":"down"}),
        ),
        (
            "switch.sync",
            json!({"sessionId":session,"sequence":1,"pressedSwitchIds":[1]}),
        ),
        (
            "switch.session.stop",
            json!({"sessionId":session,"sequence":1}),
        ),
        ("connection.disconnecting", json!({})),
    ];
    for spec in COMMANDS.iter().filter(|spec| spec.route == Route::Desktop) {
        let (_, payload) = samples
            .iter()
            .find(|(name, _)| *name == spec.name)
            .unwrap_or_else(|| panic!("{} needs a sample payload here", spec.name));
        assert!(valid_desktop_payload(spec.name, payload), "{}", spec.name);
    }
    for (name, _) in &samples {
        assert!(is_desktop_command(name), "{name} is not a desktop command");
    }
    assert!(!valid_desktop_payload("unknown.command", &json!({})));
}
