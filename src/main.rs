mod app;
mod config;
mod core;
mod img;
mod render;
mod widgets;
mod x11;

fn main() {
    if let Err(e) = app::run() {
        eprintln!("nspanel: fatal error: {e}");
        std::process::exit(1);
    }
}