mod clipboard;
mod content;
mod history;
mod paste;
mod ui;
mod worker;
fn main() -> glib::ExitCode {
    ui::run()
}
