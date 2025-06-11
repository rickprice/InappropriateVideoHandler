use anyhow::{anyhow, Result};
use std::ffi::CStr;
use std::ptr;
use x11::xlib::*;

pub struct WindowMonitor {
    display: *mut Display,
}

impl WindowMonitor {
    pub fn new() -> Result<Self> {
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(anyhow!("Failed to open X11 display"));
            }
            Ok(WindowMonitor { display })
        }
    }

    pub fn get_active_window_title(&self) -> Result<String> {
        unsafe {
            let root = XDefaultRootWindow(self.display);
            let mut window: Window = 0;
            let mut revert_to: i32 = 0;

            XGetInputFocus(self.display, &mut window, &mut revert_to);

            if window == 0 || window == root {
                return Ok(String::new());
            }

            self.get_window_title(window)
        }
    }

    pub fn get_all_window_titles(&self) -> Result<Vec<String>> {
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

            let mut titles = Vec::new();

            for i in 0..nchildren {
                let window = *children.offset(i as isize);
                if let Ok(title) = self.get_window_title(window) {
                    if !title.is_empty() {
                        titles.push(title);
                    }
                }
            }

            if !children.is_null() {
                XFree(children as *mut _);
            }

            Ok(titles)
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
