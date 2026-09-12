//! Compatibility commands and desktop adapters for the point technique.
use crate::{
    display_navigation::{self, Display},
    point_scan::{Config, Engine, Phase},
    scanning::Rect,
    scanning_runtime::{self, Adapter},
};
use tauri::AppHandle;
pub struct PointScan;
pub type Controller = scanning_runtime::Controller<PointScan>;
pub type View = scanning_runtime::View<Config, Phase>;
pub fn configure(app: &AppHandle, config: Config, enabled: bool) -> Result<View, String> {
    scanning_runtime::configure::<PointScan>(app, config, enabled)
}
pub fn install(app: &AppHandle) {
    scanning_runtime::install::<PointScan>(app);
}
impl Adapter for PointScan {
    type Config = Config;
    type Technique = Engine;
    type Environment = Display;
    const EVENT: &'static str = "point-scan-changed";
    const FILE: &'static str = "point-scan.json";
    fn validate(config: &Config) -> Result<(), String> {
        config.validate()
    }
    fn switches(config: &Config) -> crate::scanning::SwitchSettings {
        config.switches()
    }
    fn create(app: &AppHandle, config: Config) -> Result<(Engine, Display), String> {
        new_engine(app, config)
    }
    fn validate_environment(app: &AppHandle, display: Option<&Display>) -> Result<(), String> {
        validate_display(app, display)
    }
    fn prepare(app: &AppHandle) -> Result<(), String> {
        crate::point_scan_prepare(app)
    }
    fn activate(app: &AppHandle, point: (i32, i32)) -> Result<(), String> {
        crate::point_scan_click(app, point)
    }
}
fn new_engine(app: &AppHandle, config: Config) -> Result<(Engine, Display), String> {
    let (cursor, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
    let display = display_navigation::current_display(cursor, &displays)
        .ok_or("No scanning display is available.")?
        .clone();
    let units = if cfg!(target_os = "windows") {
        display.scale_factor
    } else {
        1.0
    };
    let e = Engine::new(
        config.point(),
        Rect {
            x: display.x.into(),
            y: display.y.into(),
            width: display.width.into(),
            height: display.height.into(),
        },
        units,
    )?;
    Ok((e, display))
}
fn validate_display(app: &AppHandle, display: Option<&Display>) -> Result<(), String> {
    if let Some(expected) = display {
        let (_, displays) = display_navigation::displays(app).map_err(|e| e.message)?;
        if !displays.contains(expected) {
            return Err("Display geometry changed. Enable point scan again.".into());
        }
    }
    Ok(())
}
