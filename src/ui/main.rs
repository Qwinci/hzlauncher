use iced::{Alignment, Command, Length, Renderer};
use iced::widget::{button, Column, container, pick_list, PickList, text};
use crate::backend::{McDownloader, McResult};
use crate::ui::manager::UiManagerWrapper;
use crate::ui::{Element, Modal};

#[derive(Debug, Clone)]
pub enum MainMessage {
	LoadVersions,
	VersionsLoaded(McResult<()>),
	VersionSelected(String),
	Play,
	PlayFinished(McResult<()>)
}

type Message = MainMessage;

pub struct MainUi {
	pub mc_manager: UiManagerWrapper,
	version_options: Vec<String>,
	selected_version: Option<String>
}

impl MainUi {
	pub fn new(client: reqwest::Client) -> (Self, Command<Message>) {
		let s = Self {
			mc_manager: UiManagerWrapper::new(McDownloader::new(client)),
			version_options: Vec::new(),
			selected_version: None
		};

		let versions_load_cmd = Command::perform(s.mc_manager.clone().load_versions(), Message::VersionsLoaded);

		(s, versions_load_cmd)
	}

	pub fn update<'a>(&mut self, modal: &mut Option<Box<dyn Fn() -> Modal<'a, Message>>>, message: Message) -> Command<Message> {
		match message {
			Message::LoadVersions => {
				Command::perform(self.mc_manager.clone().load_versions(), Message::VersionsLoaded)
			}
			Message::VersionsLoaded(versions) => {
				if let Err(err) = versions {
					*modal = Some(Box::new(move || Modal::with_foot(
						text(err.to_string()).into(),
						button(text("Retry")).on_press(Message::LoadVersions).into()
					)));
				} else {
					let guard = self.mc_manager.inner.blocking_lock();
					let versions = guard.versions.as_ref().unwrap();
					self.version_options = versions.versions.iter().map(|v| v.id.clone()).collect();
					self.selected_version = Some(versions.latest.release.clone());
				}
				Command::none()
			}
			Message::VersionSelected(version) => {
				self.selected_version = Some(version);
				Command::none()
			}
			Message::Play => {
				Command::perform(self.mc_manager.clone().play_version(self.selected_version.as_ref().unwrap().clone()), Message::PlayFinished)
			}
			MainMessage::PlayFinished(res) => {
				Command::none()
			}
		}
	}

	pub fn view(&self) -> Element<'_, MainMessage> {
		let mut play_button = button("Play");

		let versions: Element<'_, MainMessage> = if !self.version_options.is_empty() {
			let version_list: PickList<'_, String, Message, Renderer> = pick_list(
				&self.version_options,
				self.selected_version.clone(),
				Message::VersionSelected
			);
			play_button = play_button.on_press(Message::Play);
			version_list.into()
		} else {
			text("Loading versions...").into()
		};

		let content = Column::new()
			.push(versions)
			.push(play_button)
			.align_items(Alignment::Center);

		container(content)
			.width(Length::Fill)
			.height(Length::Fill)
			.center_x()
			.center_y()
			.into()
	}
}
