use crate::{
	EditorExtension, EditorOwned, EditorUi, ProjectSettings, SettingChanged,
	settings::LogLevelSetting,
	util::log::{EventCollectorHandle, LogLevel},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use strum::IntoEnumIterator;
use uuid::{Uuid, uuid};

#[derive(Default)]
pub struct LogsUiExtension;

impl EditorExtension for LogsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<LogUi>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.add_observer(on_log_level_changed.pipe(apply_new_level))
			.add_systems(Startup, load_log_level.pipe(apply_new_level));
	}
}

#[derive(Default, Component, Reflect)]
#[require(EditorOwned)]
pub struct LogUi {
	log_level: LogLevel,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	project_settings: ProjectSettings<'w, 's>,
	log_collector: Res<'w, EventCollectorHandle>,
}

impl EditorUi for LogUi {
	const NAME: &str = stringify!(Logs);
	const ID: Uuid = uuid!("22329413-2eff-4b95-85ad-d9b6656c9d76");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self::default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		self.log_level_selector(ui, &mut params);

		ui.separator();

		ui.add(egui_tracing::Logs::new(params.log_collector.lock().clone()));
	}
}

impl LogUi {
	fn log_level_selector(&mut self, ui: &mut egui::Ui, params: &mut Params) {
		ui.push_id("log-level-selector", |ui| {
			let previous = self.log_level;
			let mut clicked = false;
			egui::ComboBox::new("log-level-selector", "Log Level").show_ui(ui, |ui| {
				for level in LogLevel::iter() {
					clicked |= ui
						.selectable_value(&mut self.log_level, level, level.to_string())
						.clicked();
				}
			});

			if clicked && previous != self.log_level {
				params
					.project_settings
					.set(LogLevelSetting, self.log_level)
					.ok();
			}
		});
	}
}

fn load_log_level(mut project_settings: ProjectSettings) -> LogLevel {
	project_settings.get(LogLevelSetting).unwrap_or_default()
}

fn on_log_level_changed(event: On<SettingChanged<LogLevelSetting>>) -> LogLevel {
	event.value
}

fn apply_new_level(log_level: In<LogLevel>, mut q_logs: Query<&mut LogUi>) {
	for mut log_ui in &mut q_logs {
		log_ui.log_level = *log_level;
	}
}
