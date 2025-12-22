use crate::{ui::EditorUi, view::cam::EditorManagedCamera};
use bevy::{
	camera::RenderTarget, ecs::system::SystemParam, prelude::*, reflect::Reflectable,
	render::render_resource::Extent3d,
};
use bevy_egui::EguiContexts;
use persistent_id::Identifiable;
use std::marker::PhantomData;

#[derive(Component, Reflect)]
pub struct EditorManagedView<C>
where
	C: Component + Reflectable,
{
	#[reflect(ignore)]
	_pd: PhantomData<C>,
}

impl<C> Default for EditorManagedView<C>
where
	C: Component + Reflectable,
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
			Option<Single<'w, 's, &'static mut Camera, With<C>>>,
			Option<Single<'w, 's, (&'static mut Camera, &'static mut EditorManagedCamera), With<C>>>,
		),
	>,
	contexts: EguiContexts<'w, 's>,
	images: ResMut<'w, Assets<Image>>,
}

impl<C> EditorUi for EditorManagedView<C>
where
	C: Component + Reflectable + Identifiable,
{
	const NAME: &str = <C as Identifiable>::TYPE_NAME;
	const ID: uuid::Uuid = <C as Identifiable>::ID;

	const CAN_CLEAR: bool = true;

	const UNIQUE: bool = true;

	const POPOUT: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, false];

	type Params<'w, 's> = Params<'w, 's, C>;

	fn init(app: &mut App) {
		app.add_observer(take_ownership_of_camera::<C>);
	}

	fn spawn(mut params: Self::Params<'_, '_>) -> Self {
		if let Some(mut managed_camera) = params.managed_camera.p0() {
			managed_camera.is_active = true;
		}

		default()
	}

	fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
		let Some(mut managed_camera) = params.managed_camera.p0() else {
			return;
		};

		managed_camera.is_active = false;
	}

	fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
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

		let egui_rect = ui.clip_rect();

		managed_camera.viewport_rect = Some(Rect::from_corners(
			Vec2::new(egui_rect.min.x, egui_rect.min.y),
			Vec2::new(egui_rect.max.x, egui_rect.max.y),
		));

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
}

fn take_ownership_of_camera<C: Component>(event: On<Add, C>, mut commands: Commands) {
	commands
		.entity(event.event_target())
		.insert(EditorManagedCamera::default());
}
