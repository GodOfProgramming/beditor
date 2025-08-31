use bevy::{
  ecs::{
    component::{Component, ComponentId},
    resource::Resource,
    world::FromWorld,
  },
  prelude::*,
  reflect::{GetTypeRegistration, Reflect},
  utils::TypeIdMap,
};
use std::any::TypeId;

use crate::{
  Editor,
  util::vfs::{Vfs, VfsPath},
};

macro_rules! impl_reg_comp {
  // Base case: stop recursion
  () => {};

  // Recursive case: implement for one tuple size, then recurse
  ($head:ident $(, $tail:ident)* ) => {
    impl< $head: RegisterableComponent, $( $tail: RegisterableComponent ),* > RegisterableComponents for ( $head, $( $tail ),* ) {
      fn register_components(world: &mut World, component_registry: &mut ComponentRegistry) {
        $head::register(world, component_registry);
        $(
          $tail::register(world, component_registry);
        )*
      }

      fn register_types(editor: &mut Editor) {
        editor.register_type::<$head>();
        $(
          editor.register_type::<$tail>();
        )*
      }
    }

    impl_reg_comp!( $( $tail ),* );
  };
}

#[derive(Default, Resource)]
pub struct ComponentRegistry {
  mapping: TypeIdMap<RegisteredComponent>,
  vfs: Vfs<TypeId>,
}

impl ComponentRegistry {
  pub fn get(&self, type_id: &TypeId) -> Option<&RegisteredComponent> {
    self.mapping.get(type_id)
  }

  pub fn len(&self) -> usize {
    self.mapping.len()
  }

  pub fn iter(&self) -> impl Iterator<Item = (&TypeId, &RegisteredComponent)> {
    self.mapping.iter()
  }

  pub fn vfs(&self) -> &Vfs<TypeId> {
    &self.vfs
  }
}

#[derive(Clone)]
pub struct RegisteredComponent {
  name: &'static str,
  type_id: TypeId,
  id: ComponentId,
  spawn_fn: fn(entity: Entity, &mut World),
}

impl RegisteredComponent {
  pub fn name(&self) -> &str {
    self.name
  }

  pub fn spawn(&self, entity: Entity, world: &mut World) {
    (self.spawn_fn)(entity, world);
  }

  pub fn type_id(&self) -> TypeId {
    self.type_id
  }

  pub fn id(&self) -> ComponentId {
    self.id
  }
}

pub trait RegisterableComponent: GetTypeRegistration + FromWorld + Component {
  fn register(world: &mut World, component_registry: &mut ComponentRegistry);
}

impl<T> RegisterableComponent for T
where
  T: Reflect + GetTypeRegistration + FromWorld + Component,
{
  fn register(world: &mut World, component_registry: &mut ComponentRegistry) {
    let name = T::get_type_registration().type_info().type_path();
    let type_id = TypeId::of::<T>();
    let id = world.register_component::<T>();

    component_registry.mapping.insert(
      type_id,
      RegisteredComponent {
        name,
        type_id,
        id,
        spawn_fn: |entity, world| {
          let comp = T::from_world(world);
          world.entity_mut(entity).insert(comp);
        },
      },
    );

    let mut path = name.split("::");

    let count = path.clone().count();

    let (path, Some(name)) = (if count == 0 {
      (Vec::new(), path.next())
    } else {
      (
        path.clone().take(count - 1).collect::<Vec<_>>(),
        path.nth(count - 1),
      )
    }) else {
      return;
    };

    let path: VfsPath<&str> = path.into();
    let dir = component_registry.vfs.create(path);
    dir.add_item(name, type_id);

    info!("==== Begin ====");
    for (p, d) in component_registry.vfs.iter() {
      info!("Registered path: {p:?}: {d:?}");
    }
    info!("==== End ====");
  }
}

pub trait RegisterableComponents {
  fn register_components(world: &mut World, component_registry: &mut ComponentRegistry);
  fn register_types(editor: &mut Editor);
}

impl_reg_comp!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
