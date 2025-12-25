use crate::{
	EditorUi, ProjectSettings, SettingChanged,
	inspector::TypeRegistryExtensions,
	settings::LogLevelSetting,
	util::log::{EventCollectorHandle, LogLevel},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Logs {
	log_level: LogLevel,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	project_settings: ProjectSettings<'w, 's>,
	log_collector: Res<'w, EventCollectorHandle>,
	type_registry: Res<'w, AppTypeRegistry>,
}

impl EditorUi for Logs {
	const NAME: &str = stringify!(Logs);
	const ID: Uuid = uuid!("22329413-2eff-4b95-85ad-d9b6656c9d76");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		app
			.add_observer(on_log_level_changed.pipe(apply_new_level))
			.add_systems(Startup, load_log_level.pipe(apply_new_level));
	}

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self::default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, mut params: Self::Params<'_, '_>) {
		self.log_level_selector(ui, &mut params);

		ui.separator();

		ui.add(egui_tracing::Logs::new(params.log_collector.lock().clone()));
	}
}

impl Logs {
	fn log_level_selector(&self, ui: &mut egui::Ui, params: &mut Params) {
		ui.push_id("log-level-selector", |ui| {
			ui.horizontal(|ui| {
				let type_registry = params.type_registry.as_ref().read();

				ui.label("Log Level");
				let mut log_level = self.log_level;
				if type_registry.ui_for_value(ui, &mut log_level) {
					params.project_settings.set(LogLevelSetting, log_level).ok();
				}
			});
		});
	}
}

fn load_log_level(mut project_settings: ProjectSettings) -> LogLevel {
	project_settings.get(LogLevelSetting).unwrap_or_default()
}

fn on_log_level_changed(event: On<SettingChanged<LogLevelSetting>>) -> LogLevel {
	event.value
}

fn apply_new_level(log_level: In<LogLevel>, mut q_logs: Query<&mut Logs>) {
	for mut log_ui in &mut q_logs {
		log_ui.log_level = *log_level;
	}
}
