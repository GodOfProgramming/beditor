pub mod components;
pub mod debug;
pub mod egui;
pub mod entity;
pub mod log;
pub mod reflection;
pub mod storage;
pub mod window;
pub mod world;

use bevy::{ecs::world::CommandQueue, prelude::*, reflect::GetTypeRegistration};
use std::{
	borrow::{Borrow, BorrowMut},
	marker::PhantomData,
};

use crate::{EditorEntity, EditorState, Simulated};

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

pub fn make_singleton<C: Component>(
	event: On<Add, C>,
	mut commands: Commands,
	q_others: Query<Entity, With<C>>,
) {
	for entity in q_others.iter().filter(|&e| e != event.event_target()) {
		commands.entity(entity).despawn();
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
	fn try_add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
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
	fn queue<R>(&mut self, f: impl FnOnce(&mut World, &mut CommandQueue) -> R) -> R {
		let world = self.borrow_mut();
		let mut queue = CommandQueue::default();
		let r = f(world, &mut queue);
		queue.apply(world);
		r
	}

	fn spawn_stateful_entity(&mut self) -> Option<Entity> {
		let world = self.borrow_mut();

		match world.state::<EditorState>() {
			EditorState::Editing => Some(world.spawn(EditorEntity).id()),
			EditorState::SimulationPrep | EditorState::Simulating(_) => Some(world.spawn(Simulated).id()),
			_ => None,
		}
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
