use crate::{ui::EditorUi, view::cam::EditorCamera};
use bevy::{
	camera::RenderTarget, ecs::system::SystemParam, prelude::*, render::render_resource::Extent3d,
};
use bevy_egui::EguiContexts;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorView;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	editor_camera: Option<Single<'w, 's, &'static mut Camera, With<EditorCamera>>>,
	contexts: EguiContexts<'w, 's>,
	images: ResMut<'w, Assets<Image>>,
}

impl EditorUi for EditorView {
	const NAME: &str = "Editor View";
	const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

	const CAN_CLEAR: bool = false;

	const UNIQUE: bool = true;

	const POPOUT: bool = false;

	// const SCROLL_BARS: [bool; 2] = [false, false];

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn on_despawn(&mut self, params: Self::Params<'_, '_>) {
		let Some(mut editor_camera) = params.editor_camera else {
			return;
		};

		editor_camera.is_active = false;
	}

	fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			editor_camera,
			contexts,
			mut images,
		} = params;

		let Some(editor_camera) = editor_camera else {
			return;
		};

		let RenderTarget::Image(target) = &editor_camera.target else {
			return;
		};

		let Some(tex) = contexts.image_id(target.handle.id()) else {
			return;
		};

		let egui_rect = ui.clip_rect();

		ui.image(egui::load::SizedTexture::new(tex, egui_rect.size()));

		let Some(image) = images.get(target.handle.id()) else {
			return;
		};

		let viewport_size = Rect {
			max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
			min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
		}
		.size()
		.as_uvec2();

		if image.size() == viewport_size {
			return;
		}

		let Some(image) = images.get_mut(target.handle.id()) else {
			return;
		};

		image.resize(Extent3d {
			width: viewport_size.x,
			height: viewport_size.y,
			depth_or_array_layers: 1,
		})
	}

	fn when_rendered(&mut self, params: Self::Params<'_, '_>) {
		let Some(mut editor_camera) = params.editor_camera else {
			return;
		};

		editor_camera.is_active = true;
	}

	fn when_not_rendered(&mut self, params: Self::Params<'_, '_>) {
		let Some(mut editor_camera) = params.editor_camera else {
			return;
		};

		editor_camera.is_active = false;
	}
}
