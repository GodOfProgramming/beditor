use crate::{
	EditorUi,
	inspector::ui::TypeRegistryExtensions,
	util::log::{ChangeLogLevelEvent, EventCollectorHandle, LogLevel, LogLevelChangedEvent},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use uuid::{Uuid, uuid};

#[derive(Default, Component, Reflect)]
pub struct Logs {
	log_level: LogLevel,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	log_collector: Res<'w, EventCollectorHandle>,
	type_registry: Res<'w, AppTypeRegistry>,
}

impl EditorUi for Logs {
	const NAME: &str = stringify!(Logs);
	const ID: Uuid = uuid!("22329413-2eff-4b95-85ad-d9b6656c9d76");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		app.add_observer(handle_log_level_changes);
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
					params.commands.trigger(ChangeLogLevelEvent::new(log_level));
				}
			});
		});
	}
}

fn handle_log_level_changes(event: On<LogLevelChangedEvent>, mut q_logs: Query<&mut Logs>) {
	for mut this in &mut q_logs {
		this.log_level = **event;
	}
}
