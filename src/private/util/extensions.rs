use crate::{
	AppSystems, EditorState, SimulationState,
	private::{EditorOwned, SimulationOwned},
};
use bevy::{
	ecs::{
		observer::IntoObserver, schedule::ScheduleLabel, system::ScheduleSystem, world::CommandQueue,
	},
	prelude::*,
};
use notify::Notification;
use std::borrow::{Borrow, BorrowMut};

pub trait AppExtensions: BorrowMut<App> {
	fn try_add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
		let app = self.borrow_mut();
		if !app.is_plugin_added::<P>() {
			app.add_plugins(plugin);
		}
		self
	}

	fn add_app_observer<M>(&mut self, observer: impl IntoObserver<M>) -> &mut Self {
		let app = self.borrow_mut();
		app.add_observer(
			observer
				.into_observer()
				.run_if(in_state(EditorState::Simulating(SimulationState::Live))),
		);
		self
	}

	fn add_app_systems<M>(
		&mut self,
		schedule: impl ScheduleLabel,
		systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
	) -> &mut Self {
		let app = self.borrow_mut();
		app.add_systems(schedule, systems.in_set(AppSystems));
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

	fn spawn_stateful_entity(&mut self) -> Option<Entity> {
		self.spawn_stateful_entity_bundle(())
	}

	fn spawn_stateful_entity_bundle(&mut self, bundle: impl Bundle) -> Option<Entity> {
		let world = self.borrow_mut();

		match world.state::<EditorState>() {
			EditorState::Editing => Some(world.spawn((EditorOwned, bundle)).id()),
			EditorState::Simulating(_) => Some(world.spawn((SimulationOwned, bundle)).id()),
			_ => None,
		}
	}

	fn notify_on_error<R, E, M, C>(
		&mut self,
		f: impl FnOnce(&mut World) -> Result<R, E>,
		errfn: impl FnOnce(&mut World, E) -> (M, Option<C>),
	) -> Result<R, ()>
	where
		M: ToString,
		C: ToString + Send + Sync + 'static,
	{
		let world = self.borrow_mut();
		match (f)(world) {
			Ok(r) => Ok(r),
			Err(err) => {
				let (msg, ctx) = (errfn)(world, err);
				let mut n = Notification::error(msg.to_string());

				if let Some(ctx) = ctx {
					n.add_context(ctx);
				}

				self.borrow_mut().trigger(n);
				Err(())
			}
		}
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
    ($f)($world, ($($name,)*));
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
      type Output<'o> = ($(Mut<'o, $name>,)*);

      fn resources_scope(world: &mut World, f: impl for<'a> FnOnce(&mut World, Self::Output<'a>)) {
        chained_resource_scope!(world, f, $($name),*);
      }
    }
  };
}

variadics_please::all_tuples!(impl_resources_mut, 1, 12, T);
