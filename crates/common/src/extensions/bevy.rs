use bevy::{asset::AssetPath, ecs::world::CommandQueue, prelude::*};
use std::borrow::{Borrow, BorrowMut};

pub trait AppExtensions: BorrowMut<App> {
	fn try_add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
		let app = self.borrow_mut();
		if !app.is_plugin_added::<P>() {
			app.add_plugins(plugin);
		}
		self
	}
}

impl<T> AppExtensions for T where T: BorrowMut<App> {}

pub trait WorldExtensions: Borrow<World> {
	fn state<S: States>(&self) -> S {
		self.borrow().resource::<State<S>>().get().clone()
	}
}

impl<T> WorldExtensions for T where T: Borrow<World> {}

pub trait WorldMutExtensions: WorldExtensions + BorrowMut<World> {
	fn queue<R>(&mut self, f: impl FnOnce(&mut World, &mut CommandQueue) -> R) -> R {
		let world = self.borrow_mut();
		let mut queue = CommandQueue::default();
		let r = f(world, &mut queue);
		queue.apply(world);
		r
	}

	fn resources_scope<R>(&mut self, f: impl for<'a> FnOnce(&mut World, R::Output<'a>))
	where
		R: MultiResource,
	{
		let world = self.borrow_mut();
		R::resources_scope(world, f);
	}
}

impl<T> WorldMutExtensions for T where T: BorrowMut<World> {}

pub trait WindowExtensions: Borrow<Window> {
	fn center(&self) -> [f32; 2] {
		let window = self.borrow();
		[window.width() / 2.0, window.height() / 2.0]
	}
}

impl<T> WindowExtensions for T where T: Borrow<Window> {}

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
