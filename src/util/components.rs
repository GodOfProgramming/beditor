use crate::util::vfs::Vfs;
use bevy::{
  ecs::{component::ComponentId, world::FromWorld},
  prelude::*,
  reflect::{GetTypeRegistration, Reflect, TypeRegistration},
  utils::TypeIdMap,
};
use std::any::TypeId;

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

  pub(crate) fn register_raw(&mut self, world: &mut World, type_registration: &TypeRegistration) {
    let Some(reflect_component) = type_registration.data::<ReflectComponent>() else {
      return;
    };

    let type_name = type_registration.type_info().type_path_table().short_path();
    let type_id = type_registration.type_id();
    let Some(module_path) = type_registration
      .type_info()
      .type_path_table()
      .module_path()
    else {
      unreachable!("Every type should have a module path");
    };

    let component_id = reflect_component.register_component(world);

    self.mapping.insert(
      type_id,
      RegisteredComponent {
        type_id,
        id: component_id,
      },
    );

    let Some(path) = self.vfs.mkdir_p(module_path.split("::"), true) else {
      return;
    };

    self.vfs.new_item(path, Name::new(type_name), type_id);

    debug!(module_path, type_name, "Registered component");
  }
}

#[derive(Clone)]
pub struct RegisteredComponent {
  type_id: TypeId,
  id: ComponentId,
}

impl RegisteredComponent {
  pub fn spawn(&self, entity: Entity, world: &mut World) {
    let app_type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = app_type_registry.read();
    let Some(type_registration) = type_registry.get(self.type_id) else {
      return;
    };

    let Some(reflect_component) = type_registration.data::<ReflectComponent>() else {
      return;
    };

    let Some(reflect_default) = type_registration.data::<ReflectDefault>() else {
      return;
    };

    let component = reflect_default.default();

    let mut entity = world.entity_mut(entity);
    reflect_component.insert(&mut entity, &*component, &type_registry);
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
    component_registry.register_raw(world, &T::get_type_registration());
  }
}

pub trait RegisterableComponents {
  fn register_components(world: &mut World, component_registry: &mut ComponentRegistry);
}

impl_reg_comp!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
