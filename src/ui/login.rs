use std::time::SystemTime;
use iced::{Alignment, Background, Color, Command, Length, theme};
use iced::widget::{button, Column, Container, container, horizontal_space, Row, text, vertical_space};
use oauth2::StandardDeviceAuthorizationResponse;
use time::macros::format_description;
use crate::backend::{finish_code_login, load_account_from_file, ms_code_login};
use crate::model::Account;
use crate::ui::{Element, Theme};

#[derive(Debug, Clone)]
pub enum LoginMessage {
	Login,
	CodeGenerated(Result<StandardDeviceAuthorizationResponse, String>),
	OpenUrl,
	CopyCode,
	LoginFinished(Result<Account, String>)
}

#[derive(PartialEq)]
enum State {
	Normal,
	Loading,
	DisplayUrl,
}

pub struct LoginUi {
	state: State,
	login_url: String,
	code: String,
	client: reqwest::Client,
	time_offset: time::UtcOffset,
	expires_at: String
}

impl LoginUi {
	pub fn new(client: &reqwest::Client, time_offset: time::UtcOffset) -> (Self, Command<LoginMessage>, Option<Account>) {
		let account = load_account_from_file();

		(Self {
			state: State::Normal,
			login_url: String::new(),
			code: String::new(),
			client: client.clone(),
			time_offset,
			expires_at: String::new()
		}, Command::none(), account)
	}

	pub fn update(&mut self, message: LoginMessage) -> Command<LoginMessage> {
		match message {
			LoginMessage::Login => {
				self.state = State::Loading;
				Command::perform(ms_code_login(self.client.clone()), LoginMessage::CodeGenerated)
			}
			LoginMessage::CodeGenerated(res) => {
				match res {
					Ok(res) => {
						self.login_url = res.verification_uri().url().to_string();
						self.code = res.user_code().secret().to_string();
						let mut expires_at: time::OffsetDateTime = (SystemTime::now() + res.expires_in()).into();
						expires_at = expires_at.to_offset(self.time_offset);
						self.expires_at = expires_at.format(format_description!("[day].[month].[year] [hour]:[minute]")).unwrap();
						self.state = State::DisplayUrl;
						Command::perform(finish_code_login(self.client.clone(), res), LoginMessage::LoginFinished)
					}
					Err(err) => {
						self.state = State::Normal;
						eprintln!("error: {}", err);
						Command::none()
					}
				}
			}
			LoginMessage::CopyCode => {
				iced::clipboard::write(self.code.clone())
			}
			LoginMessage::OpenUrl => {
				webbrowser::open(&self.login_url).ok();
				Command::none()
			}
			_ => unreachable!()
		}
	}

	pub fn view(&self) -> Element<'_, LoginMessage> {
		let mut login_button = button(text("Login with Microsoft"));
		if self.state == State::Normal {
			login_button = login_button.on_press(LoginMessage::Login);
		}
		let status: Element<'_, LoginMessage> = match self.state {
			State::Normal => text("").into(),
			State::Loading => text("Logging in...").into(),
			State::DisplayUrl => {
				let begin = text("Open the following link in your browser");
				let url = button(self.login_url.as_str()).on_press(LoginMessage::OpenUrl)
					.style(theme::Button::Text);
				let code_text = text("and enter code");
				let code = text(&self.code);
				let copy_button = button("Copy code to clipboard").on_press(LoginMessage::CopyCode);
				let expiry_time = text(format!("Code expires at {}", self.expires_at));
				Column::new()
					.push(begin)
					.push(url)
					.push(code_text)
					.push(code)
					.push(copy_button)
					.push(expiry_time)
					.align_items(Alignment::Center)
					.into()
			}
		};

		let content = Container::new(Column::new()
			.push(login_button)
			.push(status)
			.padding([0, 0, 20, 0])
			.align_items(Alignment::Center))
			.style(|_theme: &Theme| {
				container::Appearance {
					background: Some(Background::Color(Color::from_rgb8(0x19, 0x1B, 0x1D))),
					..Default::default()
				}
			})
			.width(Length::FillPortion(2))
			.height(Length::FillPortion(2))
			.center_x()
			.center_y();

		let whole_content = Column::new()
			.push(vertical_space(Length::FillPortion(2)))
			.push(Row::new()
				.push(horizontal_space(Length::FillPortion(2)))
				.push(content)
				.push(horizontal_space(Length::FillPortion(2)))
				.height(Length::FillPortion(2))
			)
			.push(vertical_space(Length::FillPortion(2)));

		Container::new(whole_content)
			.width(Length::Fill)
			.height(Length::Fill)
			.center_x()
			.center_y()
			.into()
	}

	pub fn reset(&mut self) {
		self.state = State::Normal;
	}
}
