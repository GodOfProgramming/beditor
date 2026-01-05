use crate::{
	EditorExtension,
	private::{EditorInternal, UserHidden, ui::misc::CenteredFileDialog},
	ui::EditorUi,
	util::egui::ContextExtensions,
};
use bevy::{
	ecs::system::SystemParam,
	prelude::*,
	render::view::screenshot::{Screenshot, save_to_disk},
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use derive_new::new;
use egui::Widget;
use uuid::uuid;

#[derive(Default)]
pub struct ImageViewerUiExtension;

impl EditorExtension for ImageViewerUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<ImageViewerUi>();
	}
}

#[derive(new, Component, Default)]
#[require(EditorInternal)]
pub struct ImageViewerUi {
	pub(crate) image_id: AssetId<Image>,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	images: ResMut<'w, Assets<Image>>,
	user_textures: ResMut<'w, EguiUserTextures>,
	screenshot_file_dialog: Local<'s, CenteredFileDialog>,
}

impl EditorUi for ImageViewerUi {
	const NAME: &str = "Image View";
	const ID: uuid::Uuid = uuid!("5cf9e67a-df8e-4070-a21f-c6301f0ce26f");

	const HIDDEN: bool = true;

	const POPOUT: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, false];

	const REOPEN_ON_STARTUP: bool = false;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			mut commands,
			mut images,
			mut screenshot_file_dialog,
			mut user_textures,
			..
		} = params;

		let Some(image) = images.get(self.image_id) else {
			ui.label("No Image Selected");
			return;
		};

		let image_size = image.size();
		let image_size_vec2 = image_size.as_vec2();
		let size_in_points = ui.ctx().to_points(image_size_vec2);
		let size_in_points = if size_in_points.is_finite() {
			size_in_points
		} else {
			image_size_vec2
		};

		let ui_area = ui.clip_rect();

		let texture_rect = egui::Rect::from_center_size(
			ui_area.center(),
			egui::vec2(size_in_points.x, size_in_points.y),
		);

		self.show(ui, texture_rect, &mut user_textures);

		screenshot_file_dialog.update(ui.ctx());

		if let Some(path) = screenshot_file_dialog.take_picked() {
			let copy = image.clone();
			let handle = images.add(copy);
			commands
				.spawn((UserHidden, Screenshot::image(handle)))
				.observe(save_to_disk(path));
		}
	}

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		params: Self::Params<'_, '_>,
		_surface: egui_dock::SurfaceIndex,
		_node: egui_dock::NodeIndex,
	) {
		let Self::Params {
			screenshot_file_dialog: mut file_dialog,
			..
		} = params;

		if ui.button("Capture").clicked() {
			file_dialog.save_file();
		}
	}
}

impl ImageViewerUi {
	pub(crate) fn show(
		&self,
		ui: &mut egui::Ui,
		location: egui::Rect,
		user_textures: &mut EguiUserTextures,
	) -> egui::InnerResponse<egui::Response> {
		ui.scope_builder(
			egui::UiBuilder::new()
				.max_rect(location)
				.layout(egui::Layout::centered_and_justified(
					egui::Direction::TopDown,
				)),
			|ui| {
				let tex = user_textures.add_image(EguiTextureHandle::Weak(self.image_id));
				let tex = egui::load::SizedTexture::new(tex, location.size());
				egui::Image::new(tex).sense(egui::Sense::all()).ui(ui)
			},
		)
	}
}
