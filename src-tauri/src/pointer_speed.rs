//! The pointer speed level shown to users.
//!
//! Settings store the pointer scale as a percentage of the base step (5 to 1350 in steps of 5).
//! Every surface shows that value as a level from 1 to 10 on a logarithmic scale: level 1 is 5%,
//! level 5 is 100% and level 10 is 1350%. This mirrors `src/settings/pointerSpeedScale.ts`.

const MIN_PERCENT: f64 = 5.0;
const NORMAL_PERCENT: f64 = 100.0;
const MAX_PERCENT: f64 = 1350.0;
/// How far one Slower or Faster press moves the level.
const TILE_STEP: f64 = 0.5;

/// The level, from 1 to 10, for a stored pointer scale percentage.
pub fn level(percent: u16) -> f64 {
    let percent = f64::from(percent).clamp(MIN_PERCENT, MAX_PERCENT);
    if percent <= NORMAL_PERCENT {
        1.0 + 4.0 * (percent / MIN_PERCENT).ln() / (NORMAL_PERCENT / MIN_PERCENT).ln()
    } else {
        5.0 + 5.0 * (percent / NORMAL_PERCENT).ln() / (MAX_PERCENT / NORMAL_PERCENT).ln()
    }
}

/// The stored pointer scale percentage nearest to a level, rounded to a supported step of 5.
pub fn percent(level: f64) -> u16 {
    let bounded = level.clamp(1.0, 10.0);
    let value = if bounded <= 5.0 {
        MIN_PERCENT * (NORMAL_PERCENT / MIN_PERCENT).powf((bounded - 1.0) / 4.0)
    } else {
        NORMAL_PERCENT * (MAX_PERCENT / NORMAL_PERCENT).powf((bounded - 5.0) / 5.0)
    };
    ((value / 5.0).round() * 5.0).clamp(MIN_PERCENT, MAX_PERCENT) as u16
}

/// The level with one decimal, as Settings shows it.
pub fn label(percent: u16) -> String {
    format!("Level {:.1}", level(percent))
}

/// The percentage after one Slower (`direction < 0`) or Faster (`direction > 0`) press.
///
/// A press moves to the next half level, so a fine-tuned value between half levels snaps to the
/// grid first. Near the slow end several half levels round to the same percentage, so a press
/// always changes the stored value by at least one step of 5.
pub fn step(current: u16, direction: i8) -> u16 {
    if direction == 0 {
        return current;
    }
    let current_level = level(current);
    // Percentages are quantised to steps of 5, so a value produced from a half level does not
    // map back to it exactly. Treat the nearest half level as the current one when it rounds to
    // the same percentage; otherwise the value is fine-tuned between half levels.
    let nearest = (current_level / TILE_STEP).round() * TILE_STEP;
    let target = if percent(nearest) == current {
        nearest + TILE_STEP * f64::from(direction.signum())
    } else if direction > 0 {
        (current_level / TILE_STEP).floor() * TILE_STEP + TILE_STEP
    } else {
        (current_level / TILE_STEP).ceil() * TILE_STEP - TILE_STEP
    };
    let next = percent(target);
    if next == current {
        if direction > 0 {
            (current + 5).min(MAX_PERCENT as u16)
        } else {
            current.saturating_sub(5).max(MIN_PERCENT as u16)
        }
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_match_the_settings_scale() {
        assert_eq!(level(5), 1.0);
        assert_eq!(level(100), 5.0);
        assert_eq!(level(1350), 10.0);
        assert_eq!(percent(1.0), 5);
        assert_eq!(percent(5.0), 100);
        assert_eq!(percent(10.0), 1350);
        // Values shared with the TypeScript scale.
        assert_eq!(percent(3.0), 20);
        assert_eq!(percent(7.5), 365);
        assert!((level(225) - 6.558).abs() < 0.001);
    }

    #[test]
    fn labels_use_one_decimal_like_settings() {
        assert_eq!(label(100), "Level 5.0");
        assert_eq!(label(225), "Level 6.6");
        assert_eq!(label(1350), "Level 10.0");
    }

    #[test]
    fn a_press_moves_half_a_level_on_the_grid() {
        assert_eq!(step(100, 1), percent(5.5));
        assert_eq!(step(100, -1), percent(4.5));
        assert_eq!(step(percent(7.5), 1), percent(8.0));
        assert_eq!(step(percent(7.5), -1), percent(7.0));
        assert_eq!(step(1350, 1), 1350);
        assert_eq!(step(1350, -1), percent(9.5));
        assert_eq!(step(5, -1), 5);
        assert_eq!(step(100, 0), 100);
    }

    #[test]
    fn a_fine_tuned_value_snaps_to_the_next_half_level() {
        // 225% is level 6.557, between 6.5 and 7.
        assert_eq!(step(225, 1), percent(7.0));
        assert_eq!(step(225, -1), percent(6.5));
    }

    #[test]
    fn every_press_changes_the_speed_until_the_range_ends() {
        let mut current = 5;
        let mut presses = 0;
        while current < 1350 {
            let next = step(current, 1);
            assert!(next > current, "{current} did not get faster");
            current = next;
            presses += 1;
        }
        assert!(presses <= 20, "{presses} presses to reach the top");
        while current > 5 {
            let next = step(current, -1);
            assert!(next < current, "{current} did not get slower");
            current = next;
        }
    }

    #[test]
    fn stepping_up_and_down_returns_to_the_same_half_level() {
        for half in 2..20 {
            let start = percent(f64::from(half) / 2.0);
            if start > 5 {
                assert_eq!(step(step(start, 1), -1), start, "round trip from {start}");
            }
        }
    }
}
