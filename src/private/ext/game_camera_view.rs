use super::camera_view;
use crate::{
	EditorExtension, EditorUi,
	private::{cam::EditorManagedCamera, ext::camera_view::CameraViewUi},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use persistent_id::Identifiable;
use std::marker::PhantomData;

pub struct GameCameraViewExtension<C: Component + Identifiable>(PhantomData<C>);

impl<C> Default for GameCameraViewExtension<C>
where
	C: Component + Identifiable,
{
	fn default() -> Self {
		Self(default())
	}
}

impl<C> EditorExtension for GameCameraViewExtension<C>
where
	C: Component + Identifiable,
{
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<GameCameraViewUi<C>>();
	}

	fn build_app(&self, app: &mut App) {
		app.add_observer(take_ownership_of_cameras::<C>);
	}
}

#[derive(SystemParam)]
pub struct Params<'w, 's, C: Component + Identifiable> {
	target_camera: Option<Single<'w, 's, Entity, With<C>>>,
	camera_view: Local<'s, CameraViewUi>,
	camera_view_params: camera_view::Params<'w, 's>,
}

#[derive(Component)]
struct GameCameraViewUi<C: Component + Identifiable>(PhantomData<C>);

impl<C> EditorUi for GameCameraViewUi<C>
where
	C: Component + Identifiable,
{
	const NAME: &str = "Game Camera";

	const ID: uuid::Uuid = C::ID;

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's, C>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self(default())
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			target_camera,
			mut camera_view,
			camera_view_params,
		} = params;

		let Some(target_cam) = target_camera else {
			return;
		};

		camera_view.entity = *target_cam;

		camera_view.ui(ui, camera_view_params);
	}
}

fn take_ownership_of_cameras<C: Component>(event: On<Add, C>, mut commands: Commands) {
	commands
		.entity(event.event_target())
		.insert(EditorManagedCamera::default());
}
