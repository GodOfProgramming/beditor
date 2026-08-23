use super::image_viewer::ImageViewerUi;
use crate::{
	EditorExtension, EditorUi,
	private::{EditorInternal, EditorInternalQuery, UserHidden},
};
use bevy::{
	camera::{RenderTarget, visibility::RenderLayers},
	ecs::system::SystemParam,
	picking::pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation},
	prelude::*,
	render::render_resource::Extent3d,
	window::PrimaryWindow,
};
use bevy_egui::EguiUserTextures;
use common::extensions::egui::ContextExtensions;
use derive_more::derive::Deref;
use uuid::{Uuid, uuid};

#[derive(Default)]
pub(crate) struct CameraViewUiExtension;

impl EditorExtension for CameraViewUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<CameraViewUi>();
	}
}

#[derive(Component, Reflect)]
#[require(EditorInternal)]
pub(crate) struct CameraViewUi {
	this_entity: Entity,
	pub(crate) target_entity: Option<Entity>,
	ignore_size_mismatch: bool,
}

impl Default for CameraViewUi {
	fn default() -> Self {
		Self {
			this_entity: Entity::PLACEHOLDER,
			target_entity: None,
			ignore_size_mismatch: false,
		}
	}
}

impl CameraViewUi {
	pub fn new(target_entity: Entity) -> Self {
		Self {
			this_entity: Entity::PLACEHOLDER,
			target_entity: Some(target_entity),
			ignore_size_mismatch: false,
		}
	}
}

#[derive(Component, Deref, Reflect)]
#[relationship_target(relationship = CameraViewPointer, linked_spawn)]
pub struct CameraViewPointers(Vec<Entity>);

#[derive(Component, Reflect)]
#[relationship(relationship_target = CameraViewPointers)]
#[require(UserHidden)]
pub struct CameraViewPointer(Entity);

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	image_viewer: Local<'s, ImageViewerUi>,
	q_targets: EditorInternalQuery<'w, 's, &'static mut RenderTarget>,
	user_textures: ResMut<'w, EguiUserTextures>,
	images: ResMut<'w, Assets<Image>>,

	q_pointers: EditorInternalQuery<'w, 's, &'static PointerId>,
	q_view_pointers: EditorInternalQuery<'w, 's, &'static CameraViewPointers>,
	pointer_inputs: MessageWriter<'w, PointerInput>,

	primary_window: Single<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,

	last_coord: Local<'s, Vec2>,

	q_render_layers: Query<'w, 's, &'static mut RenderLayers>,
	new_render_layer: Local<'s, String>,
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
		Self::default()
	}

	fn init(&mut self, this_entity: Entity, mut params: Self::Params<'_, '_>) {
		params.commands.spawn((
			PointerId::Custom(Uuid::new_v4()),
			PointerLocation::default(),
			CameraViewPointer(this_entity),
		));
		self.this_entity = this_entity;
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			commands: _,
			mut image_viewer,
			mut q_targets,
			mut user_textures,
			mut images,
			q_pointers,
			q_view_pointers,
			primary_window,
			mut pointer_inputs,
			mut last_coord,
			..
		} = params;

		let Some(target) = self.target_entity.and_then(|e| q_targets.get_mut(e).ok()) else {
			ui.label("No camera selected");
			return;
		};

		let Some(handle) = &target.as_image() else {
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

		let inner_response = image_viewer.show(ui, texture_rect, &mut user_textures);

		let egui::InnerResponse {
			inner: image_response,
			..
		} = inner_response;

		let texture_coords = image_response.hover_pos().map(|hp| {
			let sf = primary_window.1.scale_factor();

			let local_x = hp.x * sf - image_response.rect.min.x;
			let local_y = hp.y * sf - image_response.rect.min.y;

			let scale_x = image_size.x as f32 / image_response.rect.width();
			let scale_y = image_size.y as f32 / image_response.rect.height();

			Vec2::new(local_x * scale_x, local_y * scale_y)
		});

		if let Some(tx_coords) = texture_coords
			&& let Ok(view_pointers) = q_view_pointers.get(self.this_entity)
		{
			for id in view_pointers.iter().filter_map(|e| q_pointers.get(e).ok()) {
				let Some(location) = target.normalize(Some(primary_window.0)).map(|rt| Location {
					target: rt,
					position: tx_coords,
				}) else {
					continue;
				};

				pointer_inputs.write(PointerInput::new(
					*id,
					location.clone(),
					PointerAction::Move {
						delta: tx_coords - *last_coord,
					},
				));
				*last_coord = tx_coords;

				let Some(action) = image_response.ctx.input(|i| {
					for (eb, bb) in [
						(egui::PointerButton::Primary, PointerButton::Primary),
						(egui::PointerButton::Secondary, PointerButton::Secondary),
						(egui::PointerButton::Middle, PointerButton::Middle),
					] {
						if i.pointer.button_pressed(eb) {
							return Some(PointerAction::Press(bb));
						}

						if i.pointer.button_released(eb) {
							return Some(PointerAction::Release(bb));
						}
					}
					None
				}) else {
					continue;
				};

				pointer_inputs.write(PointerInput::new(*id, location, action));
			}
		}

		if !self.ignore_size_mismatch {
			let ui_viewport_size = ui.ctx().to_pixels(ui_area.size());
			let ui_viewport_size = Vec2::new(ui_viewport_size.x, ui_viewport_size.y).as_uvec2();

			if ui_viewport_size == UVec2::ZERO || image_size == ui_viewport_size {
				return;
			}

			let Some(mut image) = images.get_mut(handle.id()) else {
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
			mut q_targets,
			mut images,
			mut q_render_layers,
			mut new_render_layer,
			..
		} = params;

		let Some(target_entity) = self.target_entity else {
			return;
		};

		if let Ok(target) = q_targets.get_mut(target_entity) {
			ui.menu_button("Aspect Ratio Overrides", |ui| {
				if ui.button("480p").clicked()
					&& let Some(image_handle) = target.as_image()
					&& let Some(mut image) = images.get_mut(image_handle.id())
				{
					self.ignore_size_mismatch = true;
					image.resize(Extent3d {
						width: 640,
						height: 480,
						depth_or_array_layers: 1,
					});
				}

				if ui.button("Clear aspect override").clicked() {
					self.ignore_size_mismatch = false;
				}
			});
		};

		if let Ok(mut render_layers) = q_render_layers.get_mut(target_entity) {
			ui.menu_button("Render Layers", |ui| {
				ui.text_edit_singleline(&mut *new_render_layer);
				if ui.button("Add Render Layer").clicked()
					&& let Ok(layer) = new_render_layer.parse()
				{
					*render_layers = render_layers.clone().with(layer)
				}

				let mut layers_to_remove = RenderLayers::none();
				for layer in render_layers.iter() {
					ui.horizontal(|ui| {
						ui.label(layer.to_string());
						if ui.button(egui_phosphor_icons::icons::X).clicked() {
							layers_to_remove = layers_to_remove.clone().with(layer);
						}
					});
				}

				if layers_to_remove != RenderLayers::none() {
					*render_layers = render_layers
						.clone()
						.union(&layers_to_remove)
						.symmetric_difference(&layers_to_remove);
				}
			});
		}
	}
}
