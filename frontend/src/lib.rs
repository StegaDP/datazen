#![forbid(unsafe_code)]

mod app;
mod i18n;
mod input;

pub fn run() -> anyhow::Result<()> {
    app::run()
}
