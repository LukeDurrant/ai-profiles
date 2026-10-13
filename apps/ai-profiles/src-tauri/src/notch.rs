// SPDX-License-Identifier: MIT

//! The notch HUD: every profile's weekly limit, at a glance, from the notch
//! of a MacBook's built-in display.
//!
//! Collapsed, the panel is the notch row: the notch plus a narrow wing either
//! side, pure black, so the panel and the camera cutout read as one shape.
//! The pointer resting in the notch or on a wing expands the panel downward
//! with one weekly meter per profile. It collapses again a moment after the
//! pointer leaves.
//!
//! Adapted from the island of antiburn (MIT, Copyright (c) 2026 Cadence AI
//! (Vic) Pty Ltd, <https://github.com/antiburn/antiburn>), without its
//! floating, docking and drag placements: here the HUD only ever lives in the
//! notch, and a Mac without one shows nothing.
//!
//! The notch is read from `NSScreen` on the main thread and cached; the
//! watcher on the async runtime only reads the cache.

use serde::Serialize;

/// The label of the HUD window. The frontend serves it `notch.html`.
pub const LABEL: &str = "notch";

/// The drawable strip either side of the notch, in logical points. Room for
/// one small figure and its padding beside the camera.
const WING: f64 = 30.0;

/// A transparent gutter either side of the panel, in logical points, where
/// the top corners curve outward into the bezel.
const FILLET: f64 = 19.0;

/// The narrowest the expanded body gets, in logical points: room for a
/// profile name and its percentage on one line.
const EXPANDED_MIN_WIDTH: f64 = 320.0;

/// How far the notch spreads over the cutout, in logical points. The cutout's
/// edge is anti-aliased; two extra points a side keep it black.
const NOTCH_SPREAD: f64 = 4.0;

/// How far the hover hotspot reaches past the notch's sides and below it, in
/// logical points. The pointer hides inside the notch, so a pointer that
/// skims it still counts.
const HOTSPOT_SIDE: f64 = 10.0;
const HOTSPOT_BELOW: f64 = 5.0;

/// The event that carries [`NotchState`] to the HUD webview.
#[cfg(target_os = "macos")]
const STATE_EVENT: &str = "notch:state";

/// The event that tells the HUD webview which limit crossed.
#[cfg(target_os = "macos")]
const ALERT_EVENT: &str = "notch:alert";

/// The event that asks the main window to show a profile.
#[cfg(target_os = "macos")]
const OPEN_PROFILE_EVENT: &str = "notch:open-profile";

/// What the panel is doing, as the webview renders it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NotchPhase {
    /// No display has a notch, or the HUD is off.
    Hidden,
    /// The notch row alone.
    Collapsed,
    /// The meters below the notch row.
    Expanded,
}

/// The geometry the webview renders, in logical points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotchState {
    pub phase: NotchPhase,
    /// The notch width, which the webview leaves empty.
    pub notch: f64,
    /// The notch height: the collapsed row.
    pub height: f64,
    pub wing: f64,
    pub fillet: f64,
    /// The width of the black panel, without the fillet gutters.
    pub body_width: f64,
    /// The most the expanded panel can grow below the notch row.
    pub body_max_height: f64,
}

impl NotchState {
    pub const fn hidden() -> Self {
        Self {
            phase: NotchPhase::Hidden,
            notch: 0.0,
            height: 0.0,
            wing: 0.0,
            fillet: 0.0,
            body_width: 0.0,
            body_max_height: 0.0,
        }
    }
}

/// A rectangle in physical desktop pixels, origin at the top-left of the
/// primary display.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// A notch in physical desktop pixels, with its display's backing scale and
/// the display's area below the menu bar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Notch {
    rect: Rect,
    scale: f64,
    available: Rect,
}

/// Where the window goes and how big it is: the origin in physical pixels,
/// the size in logical points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Frame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Notch {
    fn width(&self) -> f64 {
        self.rect.width / self.scale
    }

    fn height(&self) -> f64 {
        self.rect.height / self.scale
    }

    /// The black panel's width in `phase`, clamped to the display.
    fn body_width(&self, phase: NotchPhase) -> f64 {
        let collapsed = self.width() + 2.0 * WING;
        let wanted = if phase == NotchPhase::Expanded {
            collapsed.max(EXPANDED_MIN_WIDTH)
        } else {
            collapsed
        };
        wanted.min(self.available.width / self.scale - 2.0 * FILLET)
    }

    /// The room below the notch row, down to the bottom of the display.
    fn body_max_height(&self) -> f64 {
        ((self.available.y + self.available.height - self.rect.y - self.rect.height) / self.scale)
            .max(0.0)
    }

    fn state(&self, phase: NotchPhase) -> NotchState {
        NotchState {
            phase,
            notch: self.width(),
            height: self.height(),
            wing: WING,
            fillet: FILLET,
            body_width: self.body_width(phase),
            body_max_height: self.body_max_height(),
        }
    }

    /// The window frame in `phase`, centred on the notch. `content` is the
    /// height the webview measured below the notch row.
    fn frame(&self, phase: NotchPhase, content: f64) -> Frame {
        let width = self.body_width(phase) + 2.0 * FILLET;
        let height = if phase == NotchPhase::Expanded {
            self.height() + content.clamp(0.0, self.body_max_height())
        } else {
            self.height()
        };
        let centre = self.rect.x + self.rect.width / 2.0;
        let right = self.available.x + self.available.width - width * self.scale;
        Frame {
            x: (centre - width * self.scale / 2.0)
                .clamp(self.available.x, right.max(self.available.x)),
            y: self.rect.y,
            width,
            height,
        }
    }

    /// True when `cursor` is in the notch or just around it.
    fn in_hotspot(&self, cursor: (f64, f64)) -> bool {
        let (x, y) = cursor;
        let side = HOTSPOT_SIDE * self.scale;
        let below = HOTSPOT_BELOW * self.scale;
        x >= self.rect.x - side
            && x < self.rect.x + self.rect.width + side
            && y >= self.rect.y
            && y < self.rect.y + self.rect.height + below
    }
}

/// The notch of a display, or `None` when the display has none.
///
/// `display` is the display frame in physical pixels. `safe_top` is the top
/// safe-area inset and `aux_left` and `aux_right` are the widths of the menu
/// bar areas either side of the camera housing, all in logical points. The
/// notch is the gap between the two areas, spread over the cutout's edge.
fn notch_rect(
    display: &Rect,
    scale: f64,
    safe_top: f64,
    aux_left: f64,
    aux_right: f64,
) -> Option<Rect> {
    if safe_top <= 0.0 || scale <= 0.0 {
        return None;
    }
    let gap = display.width / scale - aux_left - aux_right;
    if gap <= 0.0 {
        return None;
    }
    Some(Rect {
        x: display.x + (aux_left - NOTCH_SPREAD / 2.0) * scale,
        y: display.y,
        width: (gap + NOTCH_SPREAD) * scale,
        height: safe_top * scale,
    })
}

/// True when `cursor` is inside `frame`.
fn in_frame(frame: &Frame, scale: f64, cursor: (f64, f64)) -> bool {
    let (x, y) = cursor;
    x >= frame.x
        && x < frame.x + frame.width * scale
        && y >= frame.y
        && y < frame.y + frame.height * scale
}

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use tauri::{
        AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
        WebviewWindowBuilder,
    };

    use super::{
        in_frame, notch_rect, Frame, Notch, NotchPhase, NotchState, Rect, ALERT_EVENT, LABEL,
        OPEN_PROFILE_EVENT, STATE_EVENT,
    };
    use crate::usage_alerts::UsageAlert;

    /// How often the watcher reads the cursor.
    const POLL: Duration = Duration::from_millis(100);
    /// How long the pointer rests on the notch before the panel opens.
    const HOVER_HOLD: Duration = Duration::from_millis(150);
    /// How long the panel stays open after the pointer leaves it.
    const LINGER: Duration = Duration::from_millis(1200);
    /// How long a usage alert holds the panel open, pointer or not.
    const ALERT_HOLD: Duration = Duration::from_secs(8);
    /// How often the watcher reads the displays again, in polls: a lid that
    /// closes or a resolution change moves or removes the notch.
    const DISPLAY_EVERY: u32 = 30;

    #[derive(Debug, Default)]
    struct Hud {
        notch: Option<Notch>,
        expanded: bool,
        /// The height the webview measured below the notch row.
        content: f64,
        /// A usage alert keeps the panel open until then.
        held_until: Option<Instant>,
    }

    static HUD: Mutex<Hud> = Mutex::new(Hud {
        notch: None,
        expanded: false,
        content: 0.0,
        held_until: None,
    });

    /// Bumped by every open and close, so a watcher of an older window stops.
    static GENERATION: AtomicU64 = AtomicU64::new(0);

    fn hud() -> MutexGuard<'static, Hud> {
        HUD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn phase(hud: &Hud) -> NotchPhase {
        match (hud.notch, hud.expanded) {
            (None, _) => NotchPhase::Hidden,
            (Some(_), false) => NotchPhase::Collapsed,
            (Some(_), true) => NotchPhase::Expanded,
        }
    }

    /// The state the webview should render now.
    pub fn state() -> NotchState {
        let hud = hud();
        match hud.notch {
            Some(notch) => notch.state(phase(&hud)),
            None => NotchState::hidden(),
        }
    }

    /// Show the HUD, or take it away. Idempotent.
    pub fn set_enabled(app: &AppHandle, enabled: bool) -> tauri::Result<()> {
        if enabled {
            open(app)
        } else {
            close(app);
            Ok(())
        }
    }

    fn open(app: &AppHandle) -> tauri::Result<()> {
        if app.get_webview_window(LABEL).is_some() {
            return Ok(());
        }
        let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("notch.html".into()))
            .title("ai-profiles notch")
            .inner_size(1.0, 1.0)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false)
            .focused(false)
            .visible(false)
            .skip_taskbar(true)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .accept_first_mouse(true)
            .build()?;
        let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
        {
            let mut hud = hud();
            hud.expanded = false;
        }
        let native = window.clone();
        window.run_on_main_thread(move || {
            raise_above_menu_bar(&native);
            refresh_notch();
            apply(&native);
        })?;
        spawn_watcher(app.clone(), window, generation);
        Ok(())
    }

    /// Close the HUD window, if it is open.
    pub fn close(app: &AppHandle) {
        GENERATION.fetch_add(1, Ordering::AcqRel);
        if let Some(window) = app.get_webview_window(LABEL) {
            let _ = window.destroy();
        }
    }

    /// Record the height the webview measured below the notch row, and fit
    /// an open panel to it.
    pub fn set_content_height(app: &AppHandle, height: f64) {
        if !height.is_finite() {
            return;
        }
        let expanded = {
            let mut hud = hud();
            hud.content = height.max(0.0);
            hud.expanded
        };
        if expanded {
            if let Some(window) = app.get_webview_window(LABEL) {
                apply(&window);
            }
        }
    }

    /// True when the panel is open in a notch, so an alert has somewhere to
    /// show.
    pub fn available() -> bool {
        hud().notch.is_some()
    }

    /// Drop the panel open for a usage alert, and tell the webview which
    /// limit crossed.
    pub fn alert(app: &AppHandle, alert: UsageAlert) {
        let Some(window) = app.get_webview_window(LABEL) else {
            return;
        };
        {
            let mut hud = hud();
            if hud.notch.is_none() {
                return;
            }
            hud.held_until = Some(Instant::now() + ALERT_HOLD);
        }
        let _ = window.emit_to(LABEL, ALERT_EVENT, alert);
        set_expanded(&window, true);
    }

    /// Bring the main window forward on profile `id`, and fold the panel.
    pub fn open_profile(app: &AppHandle, id: String) {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.unminimize();
            let _ = main.show();
            let _ = main.set_focus();
        }
        let _ = app.emit_to("main", OPEN_PROFILE_EVENT, id);
        if let Some(window) = app.get_webview_window(LABEL) {
            set_expanded(&window, false);
        }
    }

    /// Put the window where the cached notch and phase say, and tell the
    /// webview.
    fn apply(window: &WebviewWindow) {
        let (notch, phase, content) = {
            let hud = hud();
            (hud.notch, phase(&hud), hud.content)
        };
        let Some(notch) = notch else {
            let _ = window.hide();
            let _ = window.emit_to(LABEL, STATE_EVENT, NotchState::hidden());
            return;
        };
        let frame = notch.frame(phase, content);
        // A collapsed panel lets clicks through to the menu bar; an open one
        // takes them, so a profile row can be clicked.
        let _ = window.set_ignore_cursor_events(phase != NotchPhase::Expanded);
        let _ = window.set_size(LogicalSize::new(frame.width, frame.height));
        let _ = window.set_position(PhysicalPosition::new(frame.x, frame.y));
        let _ = window.emit_to(LABEL, STATE_EVENT, notch.state(phase));
        show_without_activating(window);
    }

    fn set_expanded(window: &WebviewWindow, expanded: bool) {
        {
            let mut hud = hud();
            if !expanded {
                hud.held_until = None;
            }
            if hud.expanded == expanded || hud.notch.is_none() {
                return;
            }
            hud.expanded = expanded;
        }
        apply(window);
    }

    /// The window frame now, for the pointer test.
    fn current_frame() -> Option<(Frame, f64)> {
        let hud = hud();
        hud.notch
            .map(|notch| (notch.frame(phase(&hud), hud.content), notch.scale))
    }

    /// Open the panel on a rest over the notch; fold it a moment after the
    /// pointer leaves. Every so often, read the displays again.
    fn spawn_watcher(app: AppHandle, window: WebviewWindow, generation: u64) {
        tauri::async_runtime::spawn(async move {
            let mut rested_since: Option<Instant> = None;
            let mut left_at: Option<Instant> = None;
            let mut polls: u32 = 0;
            loop {
                tokio::time::sleep(POLL).await;
                if GENERATION.load(Ordering::Acquire) != generation
                    || app.get_webview_window(LABEL).is_none()
                {
                    return;
                }
                polls = polls.wrapping_add(1);
                if polls.is_multiple_of(DISPLAY_EVERY) {
                    let native = window.clone();
                    let _ = window.run_on_main_thread(move || {
                        if refresh_notch() {
                            apply(&native);
                        }
                    });
                }
                let Ok(cursor) = window.cursor_position() else {
                    continue;
                };
                let cursor = (cursor.x, cursor.y);
                let (notch, expanded, held) = {
                    let hud = hud();
                    let held = hud.held_until.is_some_and(|until| Instant::now() < until);
                    (hud.notch, hud.expanded, held)
                };
                let Some(notch) = notch else {
                    rested_since = None;
                    left_at = None;
                    continue;
                };
                let in_frame =
                    current_frame().is_some_and(|(frame, scale)| in_frame(&frame, scale, cursor));
                let near = notch.in_hotspot(cursor) || in_frame;
                if expanded {
                    rested_since = None;
                    if near || held {
                        left_at = None;
                    } else if left_at.get_or_insert_with(Instant::now).elapsed() >= LINGER {
                        left_at = None;
                        set_expanded(&window, false);
                    }
                } else {
                    left_at = None;
                    if !near {
                        rested_since = None;
                    } else if rested_since.get_or_insert_with(Instant::now).elapsed() >= HOVER_HOLD
                    {
                        rested_since = None;
                        set_expanded(&window, true);
                    }
                }
            }
        });
    }

    /// Bring the panel on screen without making ai-profiles the active app.
    ///
    /// `WebviewWindow::show` makes the window key, which activates the app
    /// and raises the main window with it. A HUD only wants to be seen.
    fn show_without_activating(window: &WebviewWindow) {
        let native = window.clone();
        let _ = window.run_on_main_thread(move || {
            use objc2_app_kit::NSWindow;

            let Ok(pointer) = native.ns_window() else {
                return;
            };
            // SAFETY: Tauri hands out the live NSWindow of this webview
            // window, and this runs on the main thread.
            let ns_window: &NSWindow = unsafe { &*pointer.cast::<NSWindow>() };
            ns_window.orderFrontRegardless();
        });
    }

    /// Lift the window over the menu bar, onto every Space, fullscreen ones
    /// included, and out of the window cycle. Main thread only.
    fn raise_above_menu_bar(window: &WebviewWindow) {
        use objc2_app_kit::{NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior};

        let Ok(pointer) = window.ns_window() else {
            return;
        };
        // SAFETY: Tauri hands out the live NSWindow of this webview window,
        // and this runs on the main thread, where AppKit wants it touched.
        let ns_window: &NSWindow = unsafe { &*pointer.cast::<NSWindow>() };
        ns_window.setLevel(NSScreenSaverWindowLevel);
        ns_window.setHasShadow(false);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }

    /// Read the notch again and cache it. Returns true when it changed. Main
    /// thread only: off it the cache keeps its value.
    fn refresh_notch() -> bool {
        let Some(read) = objc2_foundation::MainThreadMarker::new().map(read_notch) else {
            return false;
        };
        let mut hud = hud();
        if hud.notch == read {
            return false;
        }
        hud.notch = read;
        if read.is_none() {
            hud.expanded = false;
        }
        true
    }

    /// The notch of the first display that has one.
    fn read_notch(mtm: objc2_foundation::MainThreadMarker) -> Option<Notch> {
        use objc2_app_kit::NSScreen;

        let screens = NSScreen::screens(mtm);
        let primary_height = screens.firstObject()?.frame().size.height;
        screens.iter().find_map(|screen| {
            let insets = screen.safeAreaInsets();
            if insets.top <= 0.0 {
                return None;
            }
            let scale = screen.backingScaleFactor();
            let display = desktop_rect(screen.frame(), primary_height, scale);
            let left = screen.auxiliaryTopLeftArea().size.width;
            let right = screen.auxiliaryTopRightArea().size.width;
            let available = desktop_rect(screen.visibleFrame(), primary_height, scale);
            notch_rect(&display, scale, insets.top, left, right).map(|rect| Notch {
                rect,
                scale,
                available,
            })
        })
    }

    /// A Cocoa screen frame as a desktop rect in physical pixels.
    ///
    /// Cocoa measures from the bottom-left of the primary display. The
    /// desktop, as the window and cursor report it, measures from the
    /// top-left of the primary display, scaled by the display's own factor.
    fn desktop_rect(frame: objc2_foundation::NSRect, primary_height: f64, scale: f64) -> Rect {
        Rect {
            x: frame.origin.x * scale,
            y: (primary_height - frame.origin.y - frame.size.height) * scale,
            width: frame.size.width * scale,
            height: frame.size.height * scale,
        }
    }
}

/// The HUD needs a notch, which only a Mac has.
#[cfg(not(target_os = "macos"))]
pub fn set_enabled(_app: &tauri::AppHandle, _enabled: bool) -> tauri::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn close(_app: &tauri::AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn state() -> NotchState {
    NotchState::hidden()
}

#[cfg(not(target_os = "macos"))]
pub fn set_content_height(_app: &tauri::AppHandle, _height: f64) {}

#[cfg(not(target_os = "macos"))]
pub fn available() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn open_profile(_app: &tauri::AppHandle, _id: String) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 14-inch MacBook Pro display at 2x: 1512 x 982 logical.
    const DISPLAY: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 3024.0,
        height: 1964.0,
    };
    const SCALE: f64 = 2.0;

    fn notch() -> Notch {
        Notch {
            rect: notch_rect(&DISPLAY, SCALE, 32.0, 656.0, 656.0).expect("a notched display"),
            scale: SCALE,
            available: Rect {
                y: 64.0,
                height: 1900.0,
                ..DISPLAY
            },
        }
    }

    #[test]
    fn the_notch_is_the_gap_between_the_menu_bar_areas_plus_the_spread() {
        let rect = notch().rect;
        assert_eq!(rect.x, (656.0 - 2.0) * SCALE);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, (1512.0 - 656.0 - 656.0 + 4.0) * SCALE);
        assert_eq!(rect.height, 32.0 * SCALE);
    }

    #[test]
    fn a_display_without_a_safe_area_has_no_notch() {
        assert_eq!(notch_rect(&DISPLAY, SCALE, 0.0, 0.0, 0.0), None);
        // Menu bar areas that meet leave no gap.
        assert_eq!(notch_rect(&DISPLAY, SCALE, 32.0, 756.0, 756.0), None);
    }

    #[test]
    fn collapsed_the_window_is_the_notch_row_with_a_wing_and_gutter_each_side() {
        let notch = notch();
        let frame = notch.frame(NotchPhase::Collapsed, 400.0);
        assert_eq!(frame.width, 204.0 + 2.0 * (WING + FILLET));
        assert_eq!(frame.height, 32.0);
        assert_eq!(frame.y, 0.0);
        // Centred on the notch.
        assert_eq!(
            frame.x + frame.width * SCALE / 2.0,
            notch.rect.x + notch.rect.width / 2.0
        );
    }

    #[test]
    fn expanded_the_window_grows_to_the_content_and_stays_centred() {
        let notch = notch();
        let frame = notch.frame(NotchPhase::Expanded, 180.0);
        assert_eq!(frame.width, EXPANDED_MIN_WIDTH + 2.0 * FILLET);
        assert_eq!(frame.height, 32.0 + 180.0);
        assert_eq!(
            frame.x + frame.width * SCALE / 2.0,
            notch.rect.x + notch.rect.width / 2.0
        );
    }

    #[test]
    fn expanded_content_is_capped_at_the_bottom_of_the_display() {
        let notch = notch();
        let frame = notch.frame(NotchPhase::Expanded, 5000.0);
        assert_eq!(frame.height, 32.0 + notch.body_max_height());
        assert_eq!(notch.body_max_height(), (1964.0 - 64.0) / SCALE);
    }

    #[test]
    fn the_hotspot_reaches_a_little_past_the_notch() {
        let notch = notch();
        let rect = notch.rect;
        assert!(notch.in_hotspot((rect.x + 10.0, 5.0)));
        assert!(notch.in_hotspot((rect.x - 19.0, 5.0)));
        assert!(!notch.in_hotspot((rect.x - 21.0, 5.0)));
        assert!(notch.in_hotspot((rect.x + 10.0, rect.height + 9.0)));
        assert!(!notch.in_hotspot((rect.x + 10.0, rect.height + 11.0)));
        assert!(!notch.in_hotspot((rect.x + 10.0, -1.0)));
    }

    #[test]
    fn the_pointer_is_in_the_frame_only_inside_its_physical_bounds() {
        let notch = notch();
        let frame = notch.frame(NotchPhase::Expanded, 100.0);
        assert!(in_frame(&frame, SCALE, (frame.x + 1.0, frame.y + 1.0)));
        let bottom = frame.y + frame.height * SCALE;
        assert!(in_frame(&frame, SCALE, (frame.x + 1.0, bottom - 1.0)));
        assert!(!in_frame(&frame, SCALE, (frame.x + 1.0, bottom + 1.0)));
        assert!(!in_frame(&frame, SCALE, (frame.x - 1.0, frame.y + 1.0)));
    }

    #[test]
    fn the_webview_state_is_in_logical_points() {
        let state = notch().state(NotchPhase::Collapsed);
        assert_eq!(state.notch, 204.0);
        assert_eq!(state.height, 32.0);
        assert_eq!(state.body_width, 204.0 + 2.0 * WING);
        let json = serde_json::to_value(state).expect("serializable");
        assert_eq!(json["phase"], "collapsed");
        assert_eq!(json["bodyWidth"], 204.0 + 2.0 * WING);
    }
}
