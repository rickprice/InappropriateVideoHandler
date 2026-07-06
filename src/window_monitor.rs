use anyhow::{anyhow, Result};
use log::{debug, info, trace};
use std::ffi::CStr;
use std::ptr;
use x11::xlib::*;

pub struct WindowMonitor {
    display: *mut Display,
    net_wm_pid: Atom,
    net_wm_name: Atom,
    utf8_string: Atom,
}

impl WindowMonitor {
    pub fn new() -> Result<Self> {
        info!("Opening X11 display");
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(anyhow!("Failed to open X11 display"));
            }
            info!("X11 display opened successfully");

            // Atoms are interned once here rather than on every window lookup:
            // XInternAtom is a round trip to the X server, and these names never change.
            let net_wm_pid = XInternAtom(display, c"_NET_WM_PID".as_ptr(), 0);
            let net_wm_name = XInternAtom(display, c"_NET_WM_NAME".as_ptr(), 0);
            let utf8_string = XInternAtom(display, c"UTF8_STRING".as_ptr(), 0);

            Ok(WindowMonitor {
                display,
                net_wm_pid,
                net_wm_name,
                utf8_string,
            })
        }
    }

    pub fn get_browser_window_titles(&self, browser_pids: &[i32]) -> Result<Vec<String>> {
        debug!("get_browser_window_titles: checking against pids {:?}", browser_pids);
        unsafe {
            let root = XDefaultRootWindow(self.display);
            let mut children: *mut Window = ptr::null_mut();
            let mut nchildren: u32 = 0;
            let mut parent: Window = 0;
            let mut root_return: Window = 0;

            let status = XQueryTree(
                self.display,
                root,
                &mut root_return,
                &mut parent,
                &mut children,
                &mut nchildren,
            );

            if status == 0 {
                return Err(anyhow!("Failed to query window tree"));
            }

            debug!("XQueryTree returned {} child window(s)", nchildren);

            let child_windows = std::slice::from_raw_parts(children, nchildren as usize);
            let mut titles = Vec::new();

            for (i, &window) in child_windows.iter().enumerate() {
                if let Some(pid) = self.get_window_pid(window) {
                    if browser_pids.contains(&pid) {
                        if let Ok(title) = self.get_window_title(window) {
                            if !title.is_empty() {
                                if log::log_enabled!(log::Level::Trace) {
                                    trace!("Browser window title (pid {}): '{}'", pid, title);
                                } else {
                                    debug!("Browser window title: '{}'", title);
                                }
                                titles.push(title);
                            }
                        }
                    } else if log::log_enabled!(log::Level::Trace) {
                        if let Ok(title) = self.get_window_title(window) {
                            if !title.is_empty() {
                                trace!("Non-browser window {} (pid {}): '{}'", i, pid, title);
                            }
                        }
                    }
                }
            }

            info!("get_browser_window_titles: {} non-empty title(s) found", titles.len());

            if !children.is_null() {
                XFree(children as *mut _);
            }

            Ok(titles)
        }
    }

    fn get_window_pid(&self, window: Window) -> Option<i32> {
        unsafe {
            if self.net_wm_pid == 0 {
                return None;
            }

            let mut actual_type: Atom = 0;
            let mut actual_format: i32 = 0;
            let mut nitems: u64 = 0;
            let mut bytes_after: u64 = 0;
            let mut prop: *mut u8 = ptr::null_mut();

            let result = XGetWindowProperty(
                self.display,
                window,
                self.net_wm_pid,
                0,
                1,
                0,
                XA_CARDINAL,
                &mut actual_type,
                &mut actual_format,
                &mut nitems,
                &mut bytes_after,
                &mut prop as *mut *mut u8,
            );

            if result != 0 || prop.is_null() || nitems == 0 {
                if !prop.is_null() {
                    XFree(prop as *mut _);
                }
                return None;
            }

            let pid = *(prop as *const u32) as i32;
            XFree(prop as *mut _);
            Some(pid)
        }
    }

    fn get_window_title(&self, window: Window) -> Result<String> {
        unsafe {
            // Try _NET_WM_NAME first (EWMH UTF-8, used by Chrome and modern apps).
            // XFetchName reads WM_NAME (legacy), which Chrome does not set.
            if self.net_wm_name != 0 && self.utf8_string != 0 {
                let mut actual_type: Atom = 0;
                let mut actual_format: i32 = 0;
                let mut nitems: u64 = 0;
                let mut bytes_after: u64 = 0;
                let mut prop: *mut u8 = ptr::null_mut();

                let result = XGetWindowProperty(
                    self.display,
                    window,
                    self.net_wm_name,
                    0,
                    1024,
                    0,
                    self.utf8_string,
                    &mut actual_type,
                    &mut actual_format,
                    &mut nitems,
                    &mut bytes_after,
                    &mut prop as *mut *mut u8,
                );

                if result == 0 && !prop.is_null() && nitems > 0 {
                    let bytes = std::slice::from_raw_parts(prop, nitems as usize);
                    let title = String::from_utf8_lossy(bytes).into_owned();
                    XFree(prop as *mut _);
                    if !title.is_empty() {
                        return Ok(title);
                    }
                } else if !prop.is_null() {
                    XFree(prop as *mut _);
                }
            }

            // Fall back to legacy WM_NAME.
            let mut name: *mut i8 = ptr::null_mut();
            let status = XFetchName(self.display, window, &mut name);

            if status == 0 || name.is_null() {
                return Ok(String::new());
            }

            let c_str = CStr::from_ptr(name);
            let title = c_str.to_string_lossy().into_owned();

            XFree(name as *mut _);

            Ok(title)
        }
    }
}

impl Drop for WindowMonitor {
    fn drop(&mut self) {
        unsafe {
            if !self.display.is_null() {
                XCloseDisplay(self.display);
            }
        }
    }
}

unsafe impl Send for WindowMonitor {}
unsafe impl Sync for WindowMonitor {}
