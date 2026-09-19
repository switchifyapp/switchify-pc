//! Compatibility commands and desktop adapters for the point technique.
use crate::{
    display_navigation::{self, Display},
    point_scan::Config,
    point_workflow::{Phase, Request, Workflow},
    scanning::Rect,
    scanning_runtime::{self, Adapter},
};
use tauri::{AppHandle, Manager};
#[derive(Clone)]
pub struct Environment {
    display: Display,
    foreground: usize,
    keyboard_area: Rect,
    handoff: Option<FocusHandoff>,
}
#[derive(Clone)]
struct FocusHandoff {
    target: usize,
    ready_at: std::time::Instant,
}
impl FocusHandoff {
    fn verify(&self, now: std::time::Instant, foreground: usize) -> Result<bool, String> {
        if now < self.ready_at {
            return Ok(false);
        }
        if foreground != self.target {
            return Err(
                "The selected target did not receive focus. Please select it again.".into(),
            );
        }
        Ok(true)
    }
}
pub struct PointScan;
pub type Controller = scanning_runtime::Controller<PointScan>;
pub type View = scanning_runtime::View<Config, Phase>;
pub fn configure(app: &AppHandle, config: Config) -> Result<View, String> {
    let previous = app.state::<Controller>().view().config;
    if config.switches().keys() != previous.switches().keys() {
        return Err("Edit key assignments in Settings → Switches.".into());
    }
    let view = scanning_runtime::configure::<PointScan>(app, config)?;
    // A mode change alters which remote assignments a live session needs.
    crate::remote_scan::apply(app);
    Ok(view)
}
pub fn pause(app: &AppHandle) {
    scanning_runtime::pause::<PointScan>(app);
}
pub fn interrupt(app: &AppHandle) {
    scanning_runtime::interrupt::<PointScan>(app);
}
pub fn install(app: &AppHandle) {
    scanning_runtime::install::<PointScan>(app);
}
impl Adapter for PointScan {
    type Config = Config;
    type Technique = Workflow;
    type Environment = Environment;
    const EVENT: &'static str = "point-scan-changed";
    const FILE: &'static str = "point-scan.json";
    fn validate(config: &Config) -> Result<(), String> {
        config.validate()
    }
    fn switches(config: &Config) -> crate::scanning::SwitchSettings {
        config.switches()
    }
    fn create(app: &AppHandle, config: Config) -> Result<(Workflow, Environment), String> {
        new_engine(app, config)
    }
    fn validate_environment(app: &AppHandle, display: Option<&Environment>) -> Result<(), String> {
        validate_display(app, display)
    }
    fn ready(app: &AppHandle) -> Result<(), String> {
        crate::point_scan_ready(app)
    }
    fn prepare(app: &AppHandle) -> Result<(), String> {
        crate::scan_executor::cleanup()?;
        crate::point_scan_prepare(app)
    }
    fn activate(app: &AppHandle, request: Request) -> Result<Option<Environment>, String> {
        crate::point_scan_ready(app)?;
        match request {
            Request::Prediction { token, index } => {
                return crate::prediction::select(token, index).map(|()| None)
            }
            Request::Keyboard(stroke) => {
                let result = crate::scan_executor::activate(request);
                crate::prediction::record(stroke, result.is_ok());
                return result.map(|()| None);
            }
            Request::OpenKeyboard { point: Some(point) } => {
                let target = crate::scan_host::target_at(point)?;
                crate::scan_executor::activate(request)?;
                let (_, mut environment) =
                    new_engine(app, app.state::<Controller>().view().config)?;
                // Allow the one requested click to establish focus before accepting
                // any key. No typing or scan advancement occurs during this handoff.
                environment.handoff = Some(FocusHandoff {
                    target,
                    ready_at: std::time::Instant::now() + std::time::Duration::from_millis(150),
                });
                return Ok(Some(environment));
            }
            Request::Setting(setting) => scanning_runtime::update_point_setting(app, setting),
            Request::Display(next) => scanning_runtime::restart_point_on_display(app, next),
            request => crate::scan_executor::activate(request),
        }
        .map(|()| None)
    }
    fn settle_environment(
        app: &AppHandle,
        environment: Option<&mut Environment>,
    ) -> Result<bool, String> {
        if let Some(environment) = environment {
            if let Some(handoff) = &environment.handoff {
                crate::point_scan_ready(app)?;
                if !handoff.verify(std::time::Instant::now(), crate::scan_host::foreground()?)? {
                    return Ok(false);
                }
                environment.foreground = handoff.target;
                environment.handoff = None;
            }
        }
        Ok(true)
    }
    fn deferred(request: &Request) -> bool {
        matches!(request, Request::Prediction { .. })
    }
    fn poll(app: &AppHandle, technique: &mut Workflow, config: &Config) {
        let enabled = technique.prediction_enabled();
        let ignored = vec![
            config.select_key.clone(),
            config.next_key.clone(),
            config.back_key.clone(),
            config.pause_key.clone(),
        ];
        crate::prediction::poll(app, technique.prediction_keyboard(), enabled, &ignored);
    }
    fn cleanup(_app: &AppHandle) -> Result<(), String> {
        crate::prediction::stop();
        crate::scan_executor::cleanup()
    }
}
fn new_engine(app: &AppHandle, config: Config) -> Result<(Workflow, Environment), String> {
    let (cursor, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
    let display = display_navigation::current_display(cursor, &displays)
        .ok_or("No scanning display is available.")?
        .clone();
    let units = if cfg!(target_os = "windows") {
        display.scale_factor
    } else {
        1.0
    };
    let mut e = Workflow::new(
        config.point(),
        Rect {
            x: display.x.into(),
            y: display.y.into(),
            width: display.width.into(),
            height: display.height.into(),
        },
        units,
    )?;
    let keyboard_area = crate::scan_host::work_area(Rect {
        x: display.x.into(),
        y: display.y.into(),
        width: display.width.into(),
        height: display.height.into(),
    })?;
    e.set_keyboard_area(keyboard_area);
    Ok((
        e,
        Environment {
            display,
            foreground: crate::scan_host::foreground()?,
            keyboard_area,
            handoff: None,
        },
    ))
}
fn validate_display(app: &AppHandle, display: Option<&Environment>) -> Result<(), String> {
    crate::point_scan_ready(app)?;
    if let Some(expected) = display {
        if crate::scan_host::foreground()? != expected.foreground {
            return Err("Foreground application changed. Select a new point.".into());
        }
        let (_, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
        if !displays.contains(&expected.display) {
            return Err("Display geometry changed. Scanning restarts.".into());
        }
        let d = &expected.display;
        if crate::scan_host::work_area(Rect {
            x: d.x.into(),
            y: d.y.into(),
            width: d.width.into(),
            height: d.height.into(),
        })? != expected.keyboard_area
        {
            return Err("Display work area changed. Scanning restarts.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_handoff_waits_and_only_accepts_the_clicked_target() {
        let start = std::time::Instant::now();
        let handoff = FocusHandoff {
            target: 42,
            ready_at: start + std::time::Duration::from_millis(150),
        };
        assert_eq!(handoff.verify(start, 7), Ok(false));
        assert_eq!(handoff.verify(start, 42), Ok(false));
        assert_eq!(handoff.verify(handoff.ready_at, 42), Ok(true));
        // A failed activation and an unrelated app stealing focus both fail closed.
        assert!(handoff.verify(handoff.ready_at, 7).is_err());
        assert!(handoff.verify(handoff.ready_at, 99).is_err());
    }
}
