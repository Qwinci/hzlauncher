#![deny(rust_2018_idioms)]

use iced::{Application, Font, Pixels, Settings, window};
use litcrypt2::use_litcrypt;
use crate::ui::Ui;

mod ui;
mod backend;
mod model;

use_litcrypt!();

fn main() -> iced::Result {
	let settings = Settings {
		id: None,
		window: window::Settings::default(),
		flags: time::UtcOffset::current_local_offset().expect("failed to determine local time offset"),
		fonts: Vec::new(),
		default_font: Font::DEFAULT,
		default_text_size: Pixels(16.0),
		antialiasing: false
	};
	Ui::run(settings)
}
