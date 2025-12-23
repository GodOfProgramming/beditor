pub mod components;
pub mod debug;
pub mod log;
pub mod reflection;
pub mod storage;

use bevy::{
	camera::visibility::{Layer as CameraLayer, RenderLayers},
	ecs::{bundle::NoBundleEffect, system::SystemParam},
	prelude::*,
	reflect::GetTypeRegistration,
	window::{CursorGrabMode, CursorIcon, CursorOptions},
};
use std::{borrow::BorrowMut, marker::PhantomData};

pub fn show_cursor(cursor: &mut CursorOptions) {
	cursor.visible = true;
	cursor.grab_mode = CursorGrabMode::None;
}

pub fn hide_cursor(cursor: &mut CursorOptions) {
	cursor.visible = false;
	cursor.grab_mode = CursorGrabMode::Locked;
}

pub fn set_cursor_icon(commands: &mut Commands, entity: Entity, cursor: impl Into<CursorIcon>) {
	commands.entity(entity).insert(cursor.into());
}

#[allow(unused)]
pub trait WindowExtensions {
	fn center(&self) -> [f32; 2];
}

impl WindowExtensions for Window {
	fn center(&self) -> [f32; 2] {
		[self.width() / 2.0, self.height() / 2.0]
	}
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct GameRenderLayer(CameraLayer);

#[derive(Component)]
#[require(RenderLayers = RenderLayers::layer(0))]
pub struct GameEntity;

#[derive(SystemParam)]
pub struct EntityManager<'w, 's> {
	commands: Commands<'w, 's>,
	render_layer: Res<'w, GameRenderLayer>,
}

impl EntityManager<'_, '_> {
	pub fn spawn(&mut self, bundle: impl Bundle) -> EntityCommands<'_> {
		let mut cmds = self
			.commands
			.spawn(RenderLayers::layer(self.render_layer.0));
		cmds.insert(bundle);
		cmds
	}

	pub fn spawn_batch<I>(&mut self, batch: I)
	where
		I: IntoIterator + Send + Sync + 'static,
		I::IntoIter: Send + Sync + 'static,
		I::Item: Bundle<Effect: NoBundleEffect>,
	{
		self
			.commands
			.spawn_batch(batch.into_iter().map(|bundle| (GameEntity, bundle)));
	}
}

pub trait AppExtensions: BorrowMut<App> {
	fn add_plugin_if_not_present<P: Plugin>(&mut self, plugin: P) -> &mut Self {
		let app = self.borrow_mut();
		if !app.is_plugin_added::<P>() {
			app.add_plugins(plugin);
		}
		self
	}

	fn register_types<T: sealed::RegisterableTypes>(&mut self) -> &mut Self {
		T::register(self.borrow_mut());
		self
	}
}

impl<T> AppExtensions for T where T: BorrowMut<App> {}

/* Individual Types */

pub trait RegisterableType {
	fn register(app: &mut App);
}

macro_rules! impl_registerable_type {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    $(#[$meta])*
    impl<$($name),*> RegisterableType for ($($name,)*)
    where
      $($name: GetTypeRegistration),*
    {
      fn register(app: &mut App) {
        $(
          app.register_type::<$name>();
        )*
      }
    }
  };
}

variadics_please::all_tuples!(impl_registerable_type, 0, 12, T);

/* Type Groups */

pub trait RegisterableTypeGroup {
	fn register(app: &mut App);
}

macro_rules! impl_registerable_type_group {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    $(#[$meta])*
    impl<$($name),*> RegisterableTypeGroup for ($($name,)*)
    where
      $($name: RegisterableType),*
    {
      fn register(app: &mut App) {
        $($name::register(app);)*
      }
    }
  };
}

variadics_please::all_tuples!(impl_registerable_type_group, 0, 12, T);

macro_rules! impl_registerable_types {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    $(#[$meta])*
    impl<$($name),*> sealed::RegisterableTypes for ($($name,)*)
    where
      $($name: sealed::RegisterableTypes),*
    {
      fn register(app: &mut App) {
        $(<$name as sealed::RegisterableTypes>::register(app);)*
      }
    }
  };
}

variadics_please::all_tuples!(impl_registerable_types, 0, 12, T);

pub struct TypeList<T: RegisterableType>(PhantomData<T>);

impl<T: RegisterableType> sealed::RegisterableTypes for TypeList<T> {
	fn register(app: &mut App) {
		T::register(app);
	}
}

pub struct TypeGroups<T: RegisterableTypeGroup>(PhantomData<T>);

impl<T: RegisterableTypeGroup> sealed::RegisterableTypes for TypeGroups<T> {
	fn register(app: &mut App) {
		T::register(app);
	}
}

mod sealed {
	use bevy::prelude::*;

	pub trait RegisterableTypes {
		fn register(app: &mut App);
	}
}
