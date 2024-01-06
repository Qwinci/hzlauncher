mod login;
mod manager;
mod main;

use std::time::SystemTime;
use iced::{Application, Command, executor, font, Length, Renderer};
use iced::widget::{button, container, text};
use iced_aw::{Card, CardStyles, modal};
use crate::backend::{refresh_mc, refresh_ms, save_account_to_file};
use crate::model::Account;
use crate::ui::login::{LoginMessage, LoginUi};
use crate::ui::main::{MainMessage, MainUi};

#[derive(Debug, Clone)]
pub enum Message {
	FontLoaded(Result<(), font::Error>),
	Login(LoginMessage),
	Main(MainMessage),
	Logout,
	AccountRefreshed(Result<Account, String>),
	ModalClose
}

#[derive(PartialEq)]
enum View {
	Loading,
	Login,
	Main,
}

struct Modal<'a, Message> {
	body: Element<'a, Message>,
	foot: Option<Element<'a, Message>>
}

impl<'a, Message> Modal<'a, Message> {
	fn new(body: Element<'a, Message>) -> Self {
		Self { body, foot: None }
	}

	fn with_foot(body: Element<'a, Message>, foot: Element<'a, Message>) -> Self {
		Self { body, foot: Some(foot) }
	}
}

pub struct Ui<'a> {
	login_ui: LoginUi,
	view: View,
	account: Option<Account>,
	client: reqwest::Client,
	modal: Option<Box<dyn Fn(&Ui<'a>) -> Modal<'a, Message>>>,
	main_modal: Option<Box<dyn Fn() -> Modal<'a, MainMessage>>>,
	main_ui: MainUi
}

pub type Element<'a, Message> = iced::Element<'a, Message, Renderer>;
pub type Theme = iced::Theme;

impl<'a> Ui<'a> {
	fn refresh_account(&mut self) -> (bool, Command<Message>) {
		if self.account.is_none() {
			return (false, Command::none());
		}

		let now = SystemTime::now();
		let acc = self.account.as_ref().unwrap();
		if now < acc.mc_creds.expires_at {
			(false, Command::none())
		} else {
			if now < acc.ms_creds.expires_at {
				(true, Command::perform(refresh_mc(self.client.clone(), self.account.take().unwrap()), Message::AccountRefreshed))
			} else {
				(true, Command::perform(refresh_ms(self.client.clone(), self.account.take().unwrap()), Message::AccountRefreshed))
			}
		}
	}
}

impl<'a> Application for Ui<'a> {
	type Executor = executor::Default;
	type Message = Message;
	type Theme = Theme;
	type Flags = time::UtcOffset;

	fn new(flags: Self::Flags) -> (Self, Command<Self::Message>) {
		let client = reqwest::Client::new();
		let (login_ui, login_cmd, account) = LoginUi::new(&client, flags);
		let view = if account.is_some() {
			View::Main
		} else {
			View::Login
		};
		let client_copy = client.clone();
		let (main_ui, main_cmd) = MainUi::new(client_copy);
		let mut s = Self {
			login_ui,
			view,
			account,
			client,
			modal: None,
			main_modal: None,
			main_ui,
		};
		let (refresh, refresh_cmd) = s.refresh_account();
		if refresh {
			s.view = View::Loading;
		} else {
			s.main_ui.mc_manager.inner.blocking_lock().account = s.account.clone();
		}
		(
			s,
			Command::batch([
				font::load(iced_aw::graphics::icons::BOOTSTRAP_FONT_BYTES).map(Message::FontLoaded),
				login_cmd.map(Message::Login),
				refresh_cmd,
				main_cmd.map(Message::Main)])
		)
	}

	fn title(&self) -> String {
		"HZLauncher".to_string()
	}

	fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
		match message {
			Message::Login(LoginMessage::LoginFinished(res)) => {
				match res {
					Ok(acc) => {
						self.account = Some(acc);
						self.view = View::Main;
						self.main_ui.mc_manager.inner.blocking_lock().account = self.account.clone();
						if let Err(err) = save_account_to_file(self.account.as_ref().unwrap()) {
							self.modal = Some(Box::new(move |_| Modal::new(
								text(format!("Failed to save account info to a file: {}", err)).into()
							)));
						}
					}
					Err(err) => {
						self.modal = Some(Box::new(move |_| Modal::new(
							text(format!("Failed to login: {}", err)).into()
						)));
						self.login_ui.reset();
					}
				}
				Command::none()
			}
			Message::Login(message) => {
				self.login_ui.update(message).map(Message::Login)
			},
			Message::Main(message) => self.main_ui.update(&mut self.main_modal, message).map(Message::Main),
			Message::AccountRefreshed(res) => {
				match res {
					Ok(acc) => {
						self.account = Some(acc);
						self.view = View::Main;
						self.main_ui.mc_manager.inner.blocking_lock().account = self.account.clone();
						if let Err(err) = save_account_to_file(self.account.as_ref().unwrap()) {
							self.modal = Some(Box::new(move |_| Modal::new(
								text(format!("Failed to save refreshed account info to a file: {}", err)).into()
							)));
						}
					}
					Err(err) => {
						self.modal = Some(Box::new(move |_| Modal::with_foot(
							text(format!("Failed to refresh account info: {}", err)).into(),
							button(text("Logout")).on_press(Message::Logout).into()
						)));
					}
				}
				Command::none()
			}
			Message::ModalClose => {
				self.modal = None;
				Command::none()
			}
			Message::FontLoaded(Ok(())) => Command::none(),
			Message::FontLoaded(Err(err)) => {
				eprintln!("error: failed to load font, some characters may appear incorrectly: {:?}", err);
				Command::none()
			}
			Message::Logout => {
				self.view = View::Login;
				self.login_ui.reset();
				Command::none()
			}
		}
	}

	fn view(&self) -> Element<'_, Message> {
		let underlay = match self.view {
			View::Loading => {
				container(text("Loading..."))
					.width(Length::Fill)
					.height(Length::Fill)
					.center_x()
					.center_y()
					.into()
			}
			View::Login => {
				self.login_ui.view().map(Message::Login)
			},
			View::Main => {
				self.main_ui.view().map(Message::Main)
			}
		};
		let overlay = if let Some(f) = &self.modal {
			let modal = f(self);
			Some(
				{
					let mut card = Card::new(
						text("Error"),
						container(modal.body)
							.width(Length::Fill)
							.height(Length::Shrink)
							.center_x()
							.center_y())
						.width(Length::Fill)
						.height(Length::Fill)
						.max_width(160.0)
						.max_height(320.0)
						.on_close(Message::ModalClose)
						.style(CardStyles::Primary);
					if let Some(foot) = modal.foot {
						card = card.foot(foot);
					}
					card
				}
			)
		} else if let Some(f) = &self.main_modal {
			let modal = f();
			Some(
				{
					let mut card = Card::new(
						text("Error"),
						container(modal.body.map(Message::Main))
							.width(Length::Fill)
							.height(Length::Shrink)
							.center_x()
							.center_y())
						.width(Length::Fill)
						.height(Length::Fill)
						.max_width(160.0)
						.max_height(320.0)
						.on_close(Message::ModalClose)
						.style(CardStyles::Primary);
					if let Some(foot) = modal.foot {
						card = card.foot(foot.map(Message::Main));
					}
					card
				}
			)
		} else {
			None
		};
		modal(underlay, overlay)
			.into()
	}

	fn theme(&self) -> Self::Theme {
		Theme::Dark
	}
}
