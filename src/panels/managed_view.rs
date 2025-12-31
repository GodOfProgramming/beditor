use crate::{
	EditorExtension, EditorUi,
	private::{EditorInternalQuery, EditorInternalSingle, EditorOwned, cam::EditorManagedCamera},
	util::egui::ContextExtensions,
};
use bevy::{
	camera::RenderTarget, ecs::system::SystemParam, prelude::*, render::render_resource::Extent3d,
};
use bevy_egui::EguiContexts;
use persistent_id::Identifiable;
use std::marker::PhantomData;

pub(crate) struct EditorManagedViewUiExtension<C>
where
	C: Component,
{
	_pd: PhantomData<C>,
}

impl<C> Default for EditorManagedViewUiExtension<C>
where
	C: Component,
{
	fn default() -> Self {
		Self { _pd: default() }
	}
}

impl<C> EditorExtension for EditorManagedViewUiExtension<C>
where
	C: Component + Identifiable,
{
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<EditorManagedViewUi<C>>();
	}

	fn build_app(&self, app: &mut App) {
		app
			.add_observer(take_ownership_of_camera::<C>)
			.add_observer(transfer_ownership_of_camera::<C>);
	}
}

#[derive(Component)]
#[require(EditorOwned)]
pub(crate) struct EditorManagedViewUi<C>
where
	C: Component,
{
	_pd: PhantomData<C>,
}

impl<C> Default for EditorManagedViewUi<C>
where
	C: Component,
{
	fn default() -> Self {
		Self { _pd: default() }
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's, C>
where
	C: Component,
{
	managed_camera: ParamSet<
		'w,
		's,
		(
			Option<EditorInternalSingle<'w, 's, &'static mut Camera, With<C>>>,
			Option<
				EditorInternalSingle<
					'w,
					's,
					(&'static mut Camera, &'static mut EditorManagedCamera),
					With<C>,
				>,
			>,
		),
	>,
	contexts: EguiContexts<'w, 's>,
	images: ResMut<'w, Assets<Image>>,
}

impl<C> Params<'_, '_, C>
where
	C: Component,
{
	pub fn has_camera(&mut self) -> bool {
		self.managed_camera.p1().is_some()
	}
}

impl<C> EditorUi for EditorManagedViewUi<C>
where
	C: Component + Identifiable,
{
	const NAME: &str = <C as Identifiable>::TYPE_NAME;
	const ID: uuid::Uuid = <C as Identifiable>::ID;

	const CAN_CLEAR: bool = true;

	const UNIQUE: bool = true;

	const POPOUT: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, false];

	type Params<'w, 's> = Params<'w, 's, C>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			mut managed_camera,
			contexts,
			mut images,
		} = params;

		let Some(mut managed_camera) = managed_camera.p1() else {
			ui.label("No camera type selected");
			return;
		};

		let (camera, managed_camera) = &mut *managed_camera;

		let RenderTarget::Image(target) = &camera.target else {
			ui.label("Camera render target is not an image");
			return;
		};

		let Some(tex) = contexts.image_id(target.handle.id()) else {
			ui.label("No image registered to egui contexts");
			return;
		};

		let Some(image) = images.get(target.handle.id()) else {
			ui.label("No image");
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

		let response = ui
			.scope_builder(
				egui::UiBuilder::new()
					.max_rect(texture_rect)
					.layout(egui::Layout::centered_and_justified(
						egui::Direction::TopDown,
					)),
				|ui| {
					ui.image(egui::load::SizedTexture::new(tex, texture_rect.size()));
				},
			)
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

			let Some(image) = images.get_mut(target.handle.id()) else {
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
		let Self::Params {
			mut managed_camera,
			mut images,
			..
		} = params;

		let Some(mut managed_camera) = managed_camera.p1() else {
			ui.label("No camera type selected");
			return;
		};

		let (camera, managed_camera) = &mut *managed_camera;

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

fn take_ownership_of_camera<C: Component>(event: On<Add, C>, mut commands: Commands) {
	commands
		.entity(event.event_target())
		.insert(EditorManagedCamera::default());
}

fn transfer_ownership_of_camera<C: Component>(
	_: On<Add, EditorManagedViewUi<C>>,
	mut commands: Commands,
	q_cameras: EditorInternalQuery<Entity, With<C>>,
) {
	for entity in q_cameras {
		commands
			.entity(entity)
			.insert(EditorManagedCamera::default());
	}
}
