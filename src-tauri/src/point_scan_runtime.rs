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
                let scope = crate::prediction::InputScope::capture();
                let result = crate::scan_executor::activate(request);
                crate::prediction::record(stroke, result.is_ok(), scope);
                return result.map(|()| None);
            }
            Request::OpenKeyboard | Request::OpenMouse => {
                crate::prediction::stop();
                crate::scan_executor::activate(request)
            }
            Request::MouseDrag => {
                let (position, _) = display_navigation::displays(app).map_err(|e| e.message)?;
                return crate::scan_executor::toggle_mouse_drag((
                    position.0.round() as i32,
                    position.1.round() as i32,
                ))
                .map(|()| None);
            }
            Request::MouseSpeed(direction) => {
                let model = app.state::<crate::state::AppModel>();
                let old = model.snapshot().settings.pointer_scale_percent;
                let next = (i16::from(old) + i16::from(direction) * 5).clamp(5, 225) as u8;
                model.apply_pointer_scale_percent(next)?;
                crate::state::emit_state(app, &model.shared);
                return Ok(None);
            }
            Request::MouseMonitor(dx, dy) => {
                let (cursor, displays) =
                    display_navigation::displays(app).map_err(|e| e.message)?;
                let source = display_navigation::current_display(cursor, &displays)
                    .ok_or("No scanning display is available.")?;
                let direction = match (dx, dy) {
                    (-1, 0) => "left",
                    (1, 0) => "right",
                    (0, -1) => "up",
                    (0, 1) => "down",
                    _ => return Err("Invalid monitor direction.".into()),
                };
                let target = display_navigation::target_center(source, &displays, direction)
                    .map_err(|e| e.message)?;
                crate::scan_executor::move_to(target)?;
                let (_, environment) = new_engine(app, app.state::<Controller>().view().config)?;
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
        technique: Option<&mut Workflow>,
    ) -> Result<bool, String> {
        if let (Some(environment), Some(technique)) = (environment, technique) {
            if technique.mouse_open() {
                let (cursor, displays) =
                    display_navigation::displays(app).map_err(|e| e.message)?;
                let current = display_navigation::current_display(cursor, &displays)
                    .ok_or("No scanning display is available.")?
                    .clone();
                let rect = Rect {
                    x: current.x.into(),
                    y: current.y.into(),
                    width: current.width.into(),
                    height: current.height.into(),
                };
                let area = crate::scan_host::work_area(rect)?;
                environment.display = current;
                environment.keyboard_area = area;
                environment.foreground = crate::scan_host::foreground()?;
                technique.set_mouse_area(area, displays.len());
                let settings = app.state::<crate::state::AppModel>().snapshot().settings;
                technique.set_mouse_settings(
                    settings.pointer_scale_percent,
                    settings.mouse_repeat_acceleration_duration_ms,
                );
            } else if technique.keyboard_open() {
                crate::point_scan_ready(app)?;
                let foreground = crate::scan_host::foreground()?;
                if environment.foreground != foreground {
                    crate::scan_executor::cleanup()?;
                    crate::prediction::reset();
                    technique.foreground_changed();
                    environment.foreground = foreground;
                }
            }
        }
        Ok(true)
    }
    fn preserve_visuals(request: &Request) -> bool {
        matches!(
            request,
            Request::Keyboard(_) | Request::Prediction { .. } | Request::MouseMove { .. }
        )
    }
    fn deferred(request: &Request) -> bool {
        matches!(request, Request::Prediction { .. })
    }
    fn poll(app: &AppHandle, technique: &mut Workflow, captured_keys: &[String]) {
        let enabled = technique.prediction_enabled();
        crate::prediction::poll(app, technique.prediction_keyboard(), enabled, captured_keys);
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
    fn typing_preserves_visuals_but_pointer_execution_hides_them() {
        assert!(PointScan::preserve_visuals(&Request::Keyboard(
            crate::scan_keyboard::Stroke {
                key: crate::scan_keyboard::Key::Character('a', 'A'),
                modifiers: [false; 4],
                caps: false,
            }
        )));
        assert!(PointScan::preserve_visuals(&Request::Prediction {
            token: 1,
            index: 0
        }));
        assert!(!PointScan::preserve_visuals(
            &crate::point_workflow::default_click((10, 20))
        ));
        assert!(!PointScan::preserve_visuals(&Request::OpenKeyboard));
        assert!(!PointScan::preserve_visuals(&Request::DragStart((10, 20))));
    }
}
