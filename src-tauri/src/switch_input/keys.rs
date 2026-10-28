/// Canonical key names used by embedded capture and persisted mappings:
/// keyboard keys and the first eight numbered buttons of any game controller.
pub fn normalize(code: &str) -> Option<String> {
    let value = match code {
        "Return" => "Enter",
        "Up" | "UpArrow" => "ArrowUp",
        "Down" | "DownArrow" => "ArrowDown",
        "Left" | "LeftArrow" => "ArrowLeft",
        "Right" | "RightArrow" => "ArrowRight",
        other => other,
    };
    let named = [
        "Space",
        "Enter",
        "Backspace",
        "Tab",
        "Escape",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Insert",
        "Delete",
    ];
    if named.contains(&value)
        || (value.len() == 1
            && value
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
        || value
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n) && value == format!("F{n}"))
        || gamepad_button(value).is_some()
    {
        Some(value.into())
    } else {
        None
    }
}

/// How many numbered controller buttons can be switches. Switch interfaces in
/// gamepad mode send buttons 1 to 4; eight covers every box found so far.
pub const GAMEPAD_BUTTONS: u8 = 8;

/// The key name for controller button `button` (1-based).
pub fn gamepad_name(button: u8) -> String {
    format!("Gamepad{button}")
}

/// How a key reads in a message: keyboard keys by name, controller buttons
/// as "Gamepad 1" to "Gamepad 8", matching the badges in Settings.
pub fn display_name(code: &str) -> String {
    match gamepad_button(code) {
        Some(n) => format!("Gamepad {n}"),
        None => code.to_string(),
    }
}

/// The 1-based button number of a gamepad key name, if it is one.
pub fn gamepad_button(name: &str) -> Option<u8> {
    let n = name.strip_prefix("Gamepad")?.parse::<u8>().ok()?;
    ((1..=GAMEPAD_BUTTONS).contains(&n) && name == gamepad_name(n)).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gamepad_buttons_one_to_eight_are_keys_and_nothing_else_is() {
        for n in 1..=GAMEPAD_BUTTONS {
            let name = gamepad_name(n);
            assert_eq!(normalize(&name).as_deref(), Some(name.as_str()));
            assert_eq!(gamepad_button(&name), Some(n));
        }
        for bad in [
            "Gamepad0",
            "Gamepad9",
            "Gamepad01",
            "Gamepad",
            "gamepad1",
            "Gamepad1 ",
        ] {
            assert_eq!(normalize(bad), None, "{bad}");
            assert_eq!(gamepad_button(bad), None, "{bad}");
        }
        assert_eq!(display_name("Gamepad3"), "Gamepad 3");
        assert_eq!(display_name("Space"), "Space");
        assert_eq!(normalize("Return").as_deref(), Some("Enter"));
        assert_eq!(normalize("F24").as_deref(), Some("F24"));
    }
}
