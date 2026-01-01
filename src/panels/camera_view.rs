use crate::{
	EditorExtension, EditorUi,
	panels::image_viewer::ImageViewerUi,
	private::{EditorInternal, EditorInternalQuery, cam::EditorManagedCamera},
	util::egui::ContextExtensions,
};
use bevy::{ecs::system::SystemParam, prelude::*, render::render_resource::Extent3d};
use bevy_egui::EguiUserTextures;
use derive_new::new;
use uuid::uuid;

#[derive(Default)]
pub(crate) struct CameraViewUiExtension;

impl EditorExtension for CameraViewUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<CameraViewUi>();
	}
}

#[derive(new, Component)]
#[require(EditorInternal)]
pub(crate) struct CameraViewUi {
	pub(crate) entity: Entity,
}

impl Default for CameraViewUi {
	fn default() -> Self {
		Self {
			entity: Entity::PLACEHOLDER,
		}
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	image_viewer: Local<'s, ImageViewerUi>,
	q_cameras: EditorInternalQuery<'w, 's, (&'static mut Camera, &'static mut EditorManagedCamera)>,
	user_textures: ResMut<'w, EguiUserTextures>,
	images: ResMut<'w, Assets<Image>>,
}

impl EditorUi for CameraViewUi {
	const NAME: &str = "Camera View";
	const ID: uuid::Uuid = uuid!("bda19494-f361-4b5b-af4e-35a6491f12e8");

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
			mut image_viewer,
			mut q_cameras,
			mut user_textures,
			mut images,
		} = params;

		let Ok((camera, mut managed_camera)) = q_cameras.get_mut(self.entity) else {
			ui.label("No camera selected");
			return;
		};

		let Some(handle) = &camera.target.as_image() else {
			ui.label("Camera render target is not an image");
			return;
		};

		let Some(image) = images.get(handle.id()) else {
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

		image_viewer.image_id = handle.id();

		let response = image_viewer
			.show(ui, texture_rect, &mut user_textures)
			.response;

		managed_camera.set_hovered(response.contains_pointer());

		let [min, max] = ui
			.ctx()
			.to_pixels_many([texture_rect.min, texture_rect.max])
			.map(|v| Vec2::new(v.x, v.y));

		let image_viewport_rect = Rect::from_corners(min, max);

		managed_camera.set_viewport(image_viewport_rect);

		if managed_camera.should_sync_to_viewport() {
			let ui_viewport_size = ui.ctx().to_pixels(ui_area.size());
			let ui_viewport_size = Vec2::new(ui_viewport_size.x, ui_viewport_size.y).as_uvec2();

			if ui_viewport_size == UVec2::ZERO || image_size == ui_viewport_size {
				return;
			}

			let Some(image) = images.get_mut(handle.id()) else {
				ui.label("No image (mut)");
				return;
			};

			image.resize(Extent3d {
				width: ui_viewport_size.x,
				height: ui_viewport_size.y,
				depth_or_array_layers: 1,
			});
		}
	}

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		params: Self::Params<'_, '_>,
		_surface: egui_dock::SurfaceIndex,
		_node: egui_dock::NodeIndex,
	) {
		let Params {
			mut q_cameras,
			mut images,
			..
		} = params;

		let Ok((camera, mut managed_camera)) = q_cameras.get_mut(self.entity) else {
			return;
		};

		managed_camera.set_ctx_menu_open(true);

		ui.menu_button("Aspect Ratio Overrides", |ui| {
			if ui.button("480p").clicked()
				&& let Some(image_handle) = camera.target.as_image()
				&& let Some(image) = images.get_mut(image_handle.id())
			{
				managed_camera.ignore_viewport_size();
				image.resize(Extent3d {
					width: 640,
					height: 480,
					depth_or_array_layers: 1,
				});
			}

			if ui.button("Clear aspect override").clicked() {
				managed_camera.sync_viewport_size();
			}
		});
	}
}
