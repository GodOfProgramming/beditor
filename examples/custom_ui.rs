use beditor::{
	prelude::*,
	uuid::{Uuid, uuid},
};
use bevy::prelude::*;
use egui_demo_lib::{View, WidgetGallery};

fn main() {
	App::new()
		.add_plugins((
			EditorPlugin::new(),
			EditorExtensionPlugin::<EditorUiPlugin>::default(),
		))
		.run();
}

#[derive(Default)]
struct EditorUiPlugin;

impl EditorExtension for EditorUiPlugin {
	fn build_editor(&self, ctx: &mut EditorExtensionContext) {
		ctx.register_ui::<CustomPanel>();
	}
}

#[derive(Reflect, Component, Default)]
struct CustomPanel(#[reflect(ignore)] WidgetGallery);

impl EditorUi for CustomPanel {
	const NAME: &str = "Custom Panel";

	const ID: Uuid = uuid!("b2d3a7ea-a68c-4788-a9e5-16b51d94ce52");

	type Params<'w, 's> = NoParams;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self::default()
	}

	fn ui(&mut self, ui: &mut bevy_egui::egui::Ui, _params: Self::Params<'_, '_>) {
		self.0.ui(ui);
	}
}
