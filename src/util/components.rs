use crate::ui::notifications::Notification;
use bevy::{
	ecs::{component::ComponentId, world::FromWorld},
	prelude::*,
	reflect::{GetTypeRegistration, Reflectable, TypeRegistration},
	utils::TypeIdMap,
};
use std::any::TypeId;
use vfs::Vfs;

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
		if type_registration.data::<ReflectFromWorld>().is_none()
			&& type_registration.data::<ReflectDefault>().is_none()
		{
			// early out on types that cannot be instantiated
			return;
		}

		let type_name = type_registration.type_info().type_path_table().short_path();
		let type_id = type_registration.type_id();
		let Some(module_path) = type_registration
			.type_info()
			.type_path_table()
			.module_path()
		else {
			return;
		};

		let Some(reflect_component) = type_registration.data::<ReflectComponent>() else {
			return;
		};

		let component_id = reflect_component.register_component(world);

		self.mapping.insert(
			type_id,
			RegisteredComponent {
				type_id,
				id: component_id,
			},
		);

		let Ok(path) = self.vfs.mkdir_p(module_path.split("::")) else {
			world.trigger(
				Notification::error(format!("Failed to register component {type_name}")).with_context(
					serde_json::json!({
						"module_path": module_path,
						"type_name": type_name,
						"reason": "Module path and type name conflict (logic error)",
					}),
				),
			);
			return;
		};

		if let Err(err) = self.vfs.new_item(path, type_name, type_id)
			&& !matches!(err, vfs::VfsError::ItemAlreadyExists(_))
		{
			world.trigger(
				Notification::error(format!("Failed to register component {type_name}")).with_context(
					serde_json::json!({
						"module_path": module_path,
						"type_name": type_name,
						"reason": err.to_string(),
					}),
				),
			);
		}
	}
}

#[derive(Clone)]
pub struct RegisteredComponent {
	type_id: TypeId,
	id: ComponentId,
}

impl RegisteredComponent {
	pub fn insert(&self, entity: Entity, world: &mut World) {
		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();
		let Some(type_registration) = type_registry.get(self.type_id) else {
			world.trigger(
				Notification::error("Failed to insert component").with_context("No type registration"),
			);
			return;
		};

		let Some(reflect_component) = type_registration.data::<ReflectComponent>() else {
			world.trigger(
				Notification::error("Failed to insert component").with_context("No ReflectComponent"),
			);
			return;
		};

		let reflect_from_world = type_registration.data::<ReflectFromWorld>();
		let reflect_default = type_registration.data::<ReflectDefault>();

		let component = match (reflect_from_world, reflect_default) {
			(Some(rfw), _) => rfw.from_world(world),
			(None, Some(rd)) => rd.default(),
			_ => {
				world.trigger(
					Notification::error("Failed to insert component")
						.with_context("No ReflectFromWorld or ReflectDefault"),
				);
				return;
			}
		};

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
	T: Reflectable + FromWorld + Component,
{
	fn register(world: &mut World, component_registry: &mut ComponentRegistry) {
		if component_registry.get(&TypeId::of::<T>()).is_some() {
			return;
		}

		{
			let app_type_registry = world.resource_mut::<AppTypeRegistry>();
			let mut type_registry = app_type_registry.write();
			type_registry.register::<T>();
		}

		let app_type_registry = world.resource::<AppTypeRegistry>().clone();
		let type_registry = app_type_registry.read();

		let registration = type_registry
			.get(TypeId::of::<T>())
			.expect("Type was just registered");

		let type_info = registration.type_info();

		let mut errors = Vec::new();

		if registration.data::<ReflectFromWorld>().is_none()
			&& registration.data::<ReflectDefault>().is_none()
		{
			errors.push("no ReflectDefault or ReflectWorld");
		}

		if registration.data::<ReflectComponent>().is_none() {
			errors.push("no ReflectComponent found");
		}

		if !errors.is_empty() {
			let type_name = type_info.type_path_table().short_path();
			let module_path = type_info.type_path_table().module_path();

			errors.push("try adding #[reflect(...)] and implementing the trait");

			let ctx = errors.join(", ");

			if let Some(module_path) = module_path {
				error!(
					ctx,
					"Failed to register component {module_path}::{type_name}",
				);
			} else {
				error!(ctx, "Failed to register component {type_name}",);
			}

			return;
		}

		component_registry.register_raw(world, registration);
	}
}

pub trait RegisterableComponents {
	fn register_components(world: &mut World, component_registry: &mut ComponentRegistry);
}

macro_rules! impl_registerable_type {
  ($(#[$meta:meta])* $($name: ident),*) => {
    #[allow(unused_variables)]
    $(#[$meta])*
    impl<$($name),*> RegisterableComponents for ($($name,)*)
    where
      $($name: RegisterableComponent),*
    {
      fn register_components(world: &mut World, component_registry: &mut ComponentRegistry) {
        $(
          $name::register(world, component_registry);
        )*
      }
    }
  };
}

variadics_please::all_tuples!(impl_registerable_type, 0, 32, T);
