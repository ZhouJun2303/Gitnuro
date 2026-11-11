//! Native geometry shared by automatic and history operation windows.

use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{LogicalSize, PhysicalPosition, WebviewWindow, WindowEvent};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Geometry {
    // Screen coordinates are physical; content size is logical for DPI changes.
    x: i32,
    y: i32,
    width: f64,
    height: f64,
    maximized: bool,
}

impl Geometry {
    fn valid(&self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && (480.0..=4000.0).contains(&self.width)
            && (360.0..=4000.0).contains(&self.height)
    }
}

fn load(path: &Path) -> Option<Geometry> {
    let geometry: Geometry = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    geometry.valid().then_some(geometry)
}

fn save(path: &Path, geometry: &Geometry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec(geometry).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn capture(window: &WebviewWindow, previous: Option<&Geometry>) -> Option<Geometry> {
    if window.is_minimized().ok()? {
        return previous.cloned();
    }
    let maximized = window.is_maximized().ok()?;
    if maximized {
        return previous.cloned().map(|mut geometry| {
            geometry.maximized = true;
            geometry
        });
    }
    let position = window.outer_position().ok()?;
    let size = window
        .inner_size()
        .ok()?
        .to_logical::<f64>(window.scale_factor().ok()?);
    let geometry = Geometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
    };
    geometry
        .valid()
        .then_some(geometry)
        .or_else(|| previous.cloned())
}

// A disconnected monitor must not leave the title bar outside all visible screens.
fn visible_position(x: i32, y: i32, areas: &[(i32, i32, u32, u32)]) -> bool {
    areas.iter().any(|&(left, top, width, height)| {
        let x = i64::from(x);
        let y = i64::from(y);
        x >= i64::from(left)
            && y >= i64::from(top)
            && x + 64 <= i64::from(left) + i64::from(width)
            && y + 32 <= i64::from(top) + i64::from(height)
    })
}

pub fn restore_and_track(window: &WebviewWindow) {
    let _ = window.set_min_size(Some(LogicalSize::new(480.0, 360.0)));
    // Keep native state separate from preferences so a stale settings form cannot erase it.
    let Ok(path) = super::settings::path().map(|path| path.with_file_name("flow-window.json"))
    else {
        return;
    };
    let saved = load(&path);
    if let Some(geometry) = &saved {
        let areas = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|monitor| {
                let area = monitor.work_area();
                (
                    area.position.x,
                    area.position.y,
                    area.size.width,
                    area.size.height,
                )
            })
            .collect::<Vec<_>>();
        if visible_position(geometry.x, geometry.y, &areas) {
            let _ = window.set_position(PhysicalPosition::new(geometry.x, geometry.y));
        } else {
            let _ = window.center();
        }
        let _ = window.set_size(LogicalSize::new(geometry.width, geometry.height));
    }
    let initial = capture(window, saved.as_ref());
    if saved.as_ref().is_some_and(|geometry| geometry.maximized) {
        let _ = window.maximize();
    }
    let geometry = Mutex::new(initial);
    let handle = window.clone();
    window.on_window_event(move |event| {
        if !matches!(
            event,
            WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::CloseRequested { .. }
                | WindowEvent::Destroyed
        ) {
            return;
        }
        let Ok(mut current) = geometry.lock() else {
            return;
        };
        if !matches!(event, WindowEvent::Destroyed) {
            *current = capture(&handle, current.as_ref());
        }
        // Includes the observer's automatic close; dragging/resizing never writes to disk.
        if matches!(
            event,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            if let Some(geometry) = current.as_ref() {
                if let Err(error) = save(&path, geometry) {
                    super::perf::write("WARN", "flow-window-geometry", &error);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_geometry_survives_a_reload_including_negative_monitor_coordinates() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("awegit-flow-window-{id}"));
        let path = directory.join("flow-window.json");
        let geometry = Geometry {
            x: -1500,
            y: 120,
            width: 1040.0,
            height: 760.0,
            maximized: true,
        };
        save(&path, &geometry).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(
            (
                restored.x,
                restored.y,
                restored.width,
                restored.height,
                restored.maximized
            ),
            (-1500, 120, 1040.0, 760.0, true)
        );
        std::fs::write(&path, b"broken").unwrap();
        assert!(load(&path).is_none());
        std::fs::write(
            &path,
            br#"{"x":0,"y":0,"width":0,"height":760,"maximized":false}"#,
        )
        .unwrap();
        assert!(load(&path).is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_saved_position_requires_a_visible_title_bar() {
        let areas = [(-1920, 0, 1920, 1040), (0, 0, 2560, 1400)];
        assert!(visible_position(-1500, 120, &areas));
        assert!(visible_position(120, 80, &areas));
        assert!(!visible_position(-3000, 120, &areas));
        assert!(!visible_position(2550, 80, &areas));
        assert!(!visible_position(120, -32000, &areas));
    }
}
