pub mod components;
pub mod debug;
pub mod egui;
pub mod entity;
pub mod log;
pub mod reflection;
pub mod storage;
pub mod window;
pub mod world;

use bevy::{
	camera::visibility::{Layer as CameraLayer, RenderLayers},
	ecs::{bundle::NoBundleEffect, system::SystemParam, world::CommandQueue},
	prelude::*,
	reflect::GetTypeRegistration,
};
use std::{
	borrow::{Borrow, BorrowMut},
	marker::PhantomData,
};

pub fn pretty_type_name<T>() -> String {
	format!("{:?}", disqualified::ShortName::of::<T>())
}
pub fn pretty_type_name_str(val: &str) -> String {
	format!("{:?}", disqualified::ShortName(val))
}

// Replace this when || becomes an operator
pub fn or(a: bool, b: bool) -> bool {
	a || b
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

#[allow(unused)]
pub trait WindowExtensions: Borrow<Window> {
	fn center(&self) -> [f32; 2] {
		let window = self.borrow();
		[window.width() / 2.0, window.height() / 2.0]
	}
}

impl<T> WindowExtensions for T where T: Borrow<Window> {}

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

pub trait WorldExtensions: BorrowMut<World> {
	fn queue(&mut self, f: impl FnOnce(&mut World, &mut CommandQueue)) {
		let world = self.borrow_mut();
		let mut queue = CommandQueue::default();
		f(world, &mut queue);
		queue.apply(world);
	}

	fn state<S: States>(&self) -> S {
		self.borrow().resource::<State<S>>().get().clone()
	}
}

impl<T> WorldExtensions for T where T: BorrowMut<World> {}

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
