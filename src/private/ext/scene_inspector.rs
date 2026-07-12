use bevy::{ecs::system::SystemParam, prelude::*};

use crate::{EditorExtension, EditorUi, private::ui::misc::CenteredFileDialog};

#[derive(Default)]
pub struct SceneInspectorUiExtension;

impl EditorExtension for SceneInspectorUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<SceneInspectorUi>();
	}
}

#[derive(Component)]
struct SceneInspectorUi;

#[derive(SystemParam)]
struct Params<'w, 's> {
	asset_server: Res<'w, AssetServer>,
	gltf_assets: Res<'w, Assets<Gltf>>,
	scene: Local<'s, Handle<Gltf>>,
	dialog: Local<'s, CenteredFileDialog>,
}

impl EditorUi for SceneInspectorUi {
	const NAME: &str = "Scene Inspector";

	const ID: uuid::Uuid = uuid::uuid!("faaf20fa-cdf3-48ec-87d9-f6c42f23bdc5");

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			asset_server,
			gltf_assets,
			mut scene,
			mut dialog,
		} = params;

		dialog.update(ui.ctx());

		match gltf_assets.get(scene.id()) {
			Some(scene) => {
				display_scene_info(ui, scene);
			}
			None => {
				if ui.button("Select File").clicked() {
					dialog.pick_file();
				}

				if let Some(file) = dialog.take_picked() {
					*scene = asset_server.load(file);
				}
			}
		}
	}
}

fn display_scene_info(ui: &mut egui::Ui, scene: &Gltf) {
	let data = format!("{scene:#?}");

	ui.label(data);
}
