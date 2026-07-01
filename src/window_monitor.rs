use anyhow::{anyhow, Result};
use std::ffi::CStr;
use std::ptr;
use x11::xlib::*;

pub struct WindowMonitor {
    display: *mut Display,
    debug_level: u8,
}

impl WindowMonitor {
    pub fn new(debug_level: u8) -> Result<Self> {
        if debug_level >= 1 {
            eprintln!("[DEBUG] Opening X11 display");
        }
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(anyhow!("Failed to open X11 display"));
            }
            if debug_level >= 1 {
                eprintln!("[DEBUG] X11 display opened successfully");
            }
            Ok(WindowMonitor { display, debug_level })
        }
    }

    #[allow(dead_code)]
    pub fn get_active_window_title(&self) -> Result<String> {
        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] get_active_window_title: querying input focus");
        }
        unsafe {
            let root = XDefaultRootWindow(self.display);
            let mut window: Window = 0;
            let mut revert_to: i32 = 0;

            XGetInputFocus(self.display, &mut window, &mut revert_to);

            if window == 0 || window == root {
                if self.debug_level >= 2 {
                    eprintln!("[DEBUG2] get_active_window_title: no focused window");
                }
                return Ok(String::new());
            }

            let title = self.get_window_title(window)?;
            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] Active window title: '{}'", title);
            }
            Ok(title)
        }
    }

    pub fn get_browser_window_titles(&self, browser_pids: &[i32]) -> Result<Vec<String>> {
        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] get_browser_window_titles: checking against pids {:?}", browser_pids);
        }
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

            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] XQueryTree returned {} child window(s)", nchildren);
            }

            let mut titles = Vec::new();

            for i in 0..nchildren {
                let window = *children.offset(i as isize);
                if let Some(pid) = self.get_window_pid(window) {
                    if browser_pids.contains(&pid) {
                        if let Ok(title) = self.get_window_title(window) {
                            if !title.is_empty() {
                                if self.debug_level >= 3 {
                                    eprintln!("[DEBUG3]   Browser window {} (pid {}): '{}'", i, pid, title);
                                }
                                titles.push(title);
                            }
                        }
                    }
                }
            }

            if self.debug_level >= 1 {
                eprintln!("[DEBUG] get_browser_window_titles: {} non-empty title(s) found", titles.len());
            }

            if !children.is_null() {
                XFree(children as *mut _);
            }

            Ok(titles)
        }
    }

    #[allow(dead_code)]
    pub fn get_all_window_titles(&self) -> Result<Vec<String>> {
        if self.debug_level >= 2 {
            eprintln!("[DEBUG2] get_all_window_titles: querying window tree");
        }
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

            if self.debug_level >= 2 {
                eprintln!("[DEBUG2] XQueryTree returned {} child window(s)", nchildren);
            }

            let mut titles = Vec::new();

            for i in 0..nchildren {
                let window = *children.offset(i as isize);
                if let Ok(title) = self.get_window_title(window) {
                    if !title.is_empty() {
                        if self.debug_level >= 3 {
                            eprintln!("[DEBUG3]   Window {}: '{}'", i, title);
                        }
                        titles.push(title);
                    }
                }
            }

            if self.debug_level >= 1 {
                eprintln!("[DEBUG] get_all_window_titles: {} non-empty title(s) found", titles.len());
            }

            if !children.is_null() {
                XFree(children as *mut _);
            }

            Ok(titles)
        }
    }

    fn get_window_pid(&self, window: Window) -> Option<i32> {
        unsafe {
            let net_wm_pid = XInternAtom(
                self.display,
                b"_NET_WM_PID\0".as_ptr() as *const i8,
                0,
            );

            if net_wm_pid == 0 {
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
                net_wm_pid,
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
