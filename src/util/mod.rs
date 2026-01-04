pub mod components;
pub mod debug;
pub mod egui;
pub mod entity;
pub mod ron;
pub mod storage;
pub mod world;

use crate::{
	EditorState,
	private::{EditorInternalQuery, EditorOwned, Simulated},
};
use bevy::{
	ecs::{
		system::{SystemParam, entity_command},
		world::CommandQueue,
	},
	prelude::*,
	reflect::GetTypeRegistration,
};
use std::{
	borrow::{Borrow, BorrowMut},
	marker::PhantomData,
};

#[derive(SystemParam)]
pub struct NoParams;

pub fn pretty_type_name<T>() -> String {
	format!("{:?}", disqualified::ShortName::of::<T>())
}

pub fn pretty_type_name_str(val: &str) -> String {
	format!("{:?}", disqualified::ShortName(val))
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
			EditorState::Editing => Some(world.spawn(EditorOwned).id()),
			EditorState::SimulationPrep | EditorState::Simulating(_) => Some(world.spawn(Simulated).id()),
			_ => None,
		}
	}

	fn resources_scope<R>(&mut self, f: impl for<'a> FnOnce(&mut World, R::Output<'a>))
	where
		R: MultiResource,
	{
		let world = self.borrow_mut();
		R::resources_scope(world, f);
	}

	fn state<S: States>(&self) -> S {
		self.borrow().resource::<State<S>>().get().clone()
	}
}

impl<T> WorldExtensions for T where T: BorrowMut<World> {}

pub trait Resources<'w, T> {
	type Output<'o>
	where
		Self: 'w,
		Self: 'o;

	fn resources(&'w self) -> Self::Output<'w>;
}

macro_rules! impl_resources {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    $(#[$meta])*
    impl<'w, W, $($name),*> Resources<'w, ($($name,)*)> for W
    where
      W: Borrow<World>,
      $($name: Resource),*
    {
      type Output<'o>
        = ($(&'o $name,)*)
      where
        Self: 'w,
        Self: 'o;

      fn resources(&'w self) -> Self::Output<'w> {
        let world = self.borrow();
        ($(world.resource::<$name>(),)*)
      }
    }
  };
}

variadics_please::all_tuples!(impl_resources, 1, 12, T);

pub trait MultiResource {
	type Output<'o>;

	fn resources_scope(world: &mut World, f: impl for<'a> FnOnce(&mut World, Self::Output<'a>));
}

macro_rules! chained_resource_scope {
  (
    $world:ident, $f:ident;
    ($($name:ident),*);
  ) => {
    ($f)($world, ($(&mut $name,)*));
  };

  (
    $world:ident, $f:ident;
    ($($resource_var:ident),*);
    $ty:ident $(, $rest:ident)*
  ) => {
    $world.resource_scope(|$world, mut $ty: Mut<$ty>| {
      chained_resource_scope!(
        $world, $f;
        ($($resource_var),*);
        $($rest),*
      );
    });
  };

  ($world:ident, $f:ident, $($ty:ident),+) => {
    chained_resource_scope!(
      $world, $f;
      ($($ty),*);
      $($ty),+
    );
  };
}

macro_rules! impl_resources_mut {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    #[allow(non_snake_case)]
    $(#[$meta])*
    impl<$($name),*> MultiResource for ($($name,)*)
    where
      $($name: Resource),*
    {
      type Output<'o> = ($(&'o mut $name,)*);

      fn resources_scope(world: &mut World, f: impl for<'a> FnOnce(&mut World, Self::Output<'a>)) {
        chained_resource_scope!(world, f, $($name),*);
      }
    }
  };
}

variadics_please::all_tuples!(impl_resources_mut, 1, 12, T);

/* Individual Types */

trait RegisterableType {
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
