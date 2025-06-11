use pkg_config;

fn main() {
    if pkg_config::Config::new().probe("x11").is_err() {
        panic!("X11 development libraries not found. Please install libx11-dev or xorg-x11-devel");
    }
}
