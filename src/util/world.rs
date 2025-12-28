use crate::inspector::errors::Error;
use bevy::{
	ecs::{
		change_detection::MutUntyped,
		world::unsafe_world_cell::{GetEntityMutByIdError, UnsafeWorldCell},
	},
	prelude::*,
	ptr::Ptr,
	reflect::{ReflectFromPtr, TypeRegistry},
};
use derive_more::derive::From;
use smallvec::{SmallVec, smallvec};
use std::any::{Any, TypeId};

pub type EntityComponent = (Entity, TypeId);

pub trait WorldView: Clone {
	fn world(&self) -> &World;
}

#[derive(Clone, Deref, From)]
pub struct ImmutableWorldView<'w> {
	world: &'w World,
}

impl<'w> WorldView for ImmutableWorldView<'w> {
	fn world(&self) -> &World {
		self.world
	}
}

impl<'w> From<&'w mut World> for ImmutableWorldView<'w> {
	fn from(world: &'w mut World) -> Self {
		Self { world }
	}
}

#[derive(Clone, Deref, DerefMut, From)]
pub struct MutableWorldView<'w> {
	world: UnsafeWorldCell<'w>,
}

impl<'w> From<&'w mut World> for MutableWorldView<'w> {
	fn from(world: &'w mut World) -> Self {
		Self {
			world: world.as_unsafe_world_cell(),
		}
	}
}

impl<'w> WorldView for MutableWorldView<'w> {
	fn world(&self) -> &World {
		unsafe { self.world.world() }
	}
}

#[derive(Clone)]
pub struct RestrictedWorldView<W>
where
	W: WorldView,
{
	world_view: W,
	resources: Allowed<TypeId>,
	components: Allowed<EntityComponent>,
}

impl<'w> From<&'w World> for RestrictedWorldView<ImmutableWorldView<'w>> {
	fn from(world: &'w World) -> Self {
		RestrictedWorldView {
			world_view: ImmutableWorldView::from(world),
			resources: Allowed::everything(),
			components: Allowed::everything(),
		}
	}
}

impl<'w> From<&'w mut World> for RestrictedWorldView<ImmutableWorldView<'w>> {
	fn from(world: &'w mut World) -> Self {
		RestrictedWorldView {
			world_view: ImmutableWorldView::from(world),
			resources: Allowed::everything(),
			components: Allowed::everything(),
		}
	}
}

impl<'w> From<&'w mut World> for RestrictedWorldView<MutableWorldView<'w>> {
	fn from(world: &'w mut World) -> Self {
		RestrictedWorldView {
			world_view: MutableWorldView::from(world),
			resources: Allowed::everything(),
			components: Allowed::everything(),
		}
	}
}

impl<W> RestrictedWorldView<W>
where
	W: WorldView,
{
	pub fn new(world_view: W) -> Self {
		Self {
			world_view,
			resources: Allowed::everything(),
			components: Allowed::everything(),
		}
	}

	pub fn world(&self) -> &World {
		self.world_view.world()
	}

	pub fn contains_entity(&self, entity: Entity) -> bool {
		self.world_view.world().entities().contains(entity)
	}

	/// Gets an immutable reference to the resource of the given type
	pub fn resource<R: Resource>(&self) -> Result<&R, Error> {
		let type_id = TypeId::of::<R>();
		if !self.allows_access_to_resource(type_id) {
			return Err(Error::NoAccessToResource);
		}

		self
			.world_view
			.world()
			.get_resource::<R>()
			.ok_or(Error::ResourceDoesNotExist)
	}

	/// Whether the resource with the given [`TypeId`] may be accessed from this world view
	pub fn allows_access_to_resource(&self, type_id: TypeId) -> bool {
		self.resources.allows_access_to(type_id)
	}

	/// Whether the given component at the entity may be accessed from this world view
	pub fn allows_access_to_component(&self, component: EntityComponent) -> bool {
		self.components.allows_access_to(component)
	}

	/// Splits this view into one view that only has access the the resource `resource` (`.0`), and the rest (`.1`).
	pub fn split_off_resource(&self, resource: TypeId) -> (Self, Self) {
		assert!(self.allows_access_to_resource(resource));

		// INVARIANTS: `self` had `resource` access, so `split` has access if we remove it from `self`
		let split = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: Allowed::allow_just(resource),
			components: Allowed::nothing(),
		};

		let rest = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: self.resources.without(resource),
			components: self.components.clone(),
		};

		(split, rest)
	}

	/// Splits this view into one view that only has access the the component `component.1` at the entity `component.0` (`.0`), and the rest (`.1`).
	pub fn split_off_component(&self, component: EntityComponent) -> (Self, Self) {
		assert!(self.allows_access_to_component(component));

		// INVARIANTS: `self` had `component` access, so `split` has access if we remove it from `self`
		let split = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: Allowed::nothing(),
			components: Allowed::allow_just(component),
		};

		let rest = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: self.resources.clone(),
			components: self.components.without(component),
		};

		(split, rest)
	}

	/// Splits this view into one view that only has access the the component-entity pairs `components` (`.0`), and the rest (`.1`)
	pub fn split_off_components(
		&self,
		components: impl Iterator<Item = EntityComponent> + Copy,
	) -> (Self, Self) {
		for component in components {
			assert!(self.allows_access_to_component(component));
		}

		// INVARIANTS: `self` had `component` access, so `split` has access if we remove it from `self`
		let split = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: Allowed::nothing(),
			components: Allowed::allow(components),
		};
		let rest = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: self.resources.clone(),
			components: self.components.without_many(components),
		};

		(split, rest)
	}

	/// Splits the world into one view which may only be used for resource access, and another which may only be used for component access.
	pub fn resources_components(&self) -> (Self, Self) {
		// INVARIANTS: `world` is `&mut` so we have access to everything
		let resources = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: Allowed::everything(),
			components: Allowed::nothing(),
		};
		let components = RestrictedWorldView {
			world_view: self.world_view.clone(),
			resources: Allowed::nothing(),
			components: Allowed::everything(),
		};

		(resources, components)
	}
}

impl<'w> RestrictedWorldView<ImmutableWorldView<'w>> {
	pub fn to_immutable<W>(other: &'w RestrictedWorldView<W>) -> Self
	where
		W: 'w + WorldView,
	{
		Self {
			world_view: ImmutableWorldView::from(other.world()),
			resources: other.resources.clone(),
			components: other.components.clone(),
		}
	}
}

impl RestrictedWorldView<MutableWorldView<'_>> {
	pub fn world_cell<'w>(&'w self) -> UnsafeWorldCell<'w> {
		self.world_view.world
	}

	/// Gets mutable reference to two resources. Panics if `R1 = R2`.
	pub fn two_resources_mut<R1: Resource, R2: Resource>(
		&mut self,
	) -> (Result<Mut<'_, R1>, Error>, Result<Mut<'_, R2>, Error>) {
		assert_ne!(TypeId::of::<R1>(), TypeId::of::<R2>());
		// SAFETY: &mut self, R1!=R2
		let r1 = unsafe { self.resource_unchecked_mut::<R1>() };
		// SAFETY: &mut self, R1!=R2
		let r2 = unsafe { self.resource_unchecked_mut::<R2>() };

		(r1, r2)
	}

	/// Gets a mutable reference to the resource of the given type
	pub fn resource_mut<R: Resource>(&mut self) -> Result<Mut<'_, R>, Error> {
		// SAFETY: &mut self
		unsafe { self.resource_unchecked_mut() }
	}

	/// Gets a mutable reference in form of a [`&mut dyn Reflect`](bevy_reflect::Reflect) to the resource given by `type_id`.
	///
	/// Returns an error if the type does not register [`Reflect`].
	///
	/// Also returns a `impl FnOnce()` to mark the value as changed.
	pub fn resource_reflect_mut_by_id(
		&mut self,
		type_id: TypeId,
		type_registry: &TypeRegistry,
	) -> Result<Mut<'_, dyn Reflect>, Error> {
		if !self.allows_access_to_resource(type_id) {
			return Err(Error::NoAccessToResource);
		}

		let component_id = self
			.world_view
			.world
			.components()
			.get_resource_id(type_id)
			.ok_or(Error::ResourceDoesNotExist)?;

		// SAFETY: we have access to `type_id` and borrow `&mut self`
		let value = unsafe {
			self
				.world_view
				.world
				.get_resource_mut_by_id(component_id)
				.ok_or(Error::ResourceDoesNotExist)?
		};

		// SAFETY: value is of type type_id
		let value = unsafe { mut_untyped_to_reflect(value, type_registry, type_id)? };

		Ok(value)
	}

	/// Gets a mutable reference in form of a [`&mut dyn Reflect`](bevy_reflect::Reflect) to a component at an entity.
	///
	/// Returns an error if the type does not register [`Reflect`].
	///
	/// Also returns a `impl FnOnce()` to mark the value as changed.
	pub fn entity_component_reflect_mut(
		&mut self,
		entity: Entity,
		component: TypeId,
		type_registry: &TypeRegistry,
	) -> Result<ReflectBorrow<'_>, Error> {
		if !self.allows_access_to_component((entity, component)) {
			return Err(Error::NoAccessToComponent(entity));
		}

		// SAFETY: this only accesses the component ID and doesn't keep any references
		let component_id = self
			.world_view
			.world
			.components()
			.get_id(component)
			.ok_or(Error::NoComponentId)?;

		let entity_ref = self
			.world_view
			.world
			.get_entity(entity)
			.map_err(|_| Error::ComponentDoesNotExist(entity))?;

		// SAFETY: we have access to (entity, component) and borrow `&mut self`
		match unsafe { entity_ref.get_mut_by_id(component_id) } {
			Ok(value) => {
				// SAFETY: value has the type of `component``
				let value = unsafe { mut_untyped_to_reflect(value, type_registry, component) }?;
				Ok(ReflectBorrow::Mutable(value))
			}
			Err(GetEntityMutByIdError::ComponentIsImmutable) => {
				// SAFETY: we have access to (entity, component) and borrow `&self`
				let value = unsafe { entity_ref.get_by_id(component_id) }
					.ok_or(Error::ComponentDoesNotExist(entity))?;
				// SAFETY: value has the type of `component``
				let value = unsafe { ptr_untyped_to_reflect(value, type_registry, component) }?;
				Ok(ReflectBorrow::Immutable(value))
			}
			Err(_) => Err(Error::ComponentDoesNotExist(entity)),
		}
	}

	// SAFETY: must ensure distinct access
	pub(crate) unsafe fn get_entity_component_reflect_unchecked(
		&self,
		entity: Entity,
		component: TypeId,
		type_registry: &TypeRegistry,
	) -> Result<Mut<'_, dyn Reflect>, Error> {
		if !self.allows_access_to_component((entity, component)) {
			return Err(Error::NoAccessToComponent(entity));
		}

		// SAFETY: this only accesses the component ID and doesn't keep any references
		let component_id = self
			.world_view
			.world
			.components()
			.get_id(component)
			.ok_or(Error::NoComponentId)?;

		// SAFETY: we have access to (entity, component) and caller ensures distinct access
		let value = unsafe {
			self
				.world_view
				.world
				.get_entity(entity)
				.map_err(|_| Error::ComponentDoesNotExist(entity))?
				.get_mut_by_id(component_id)
				.map_err(|_| Error::ComponentDoesNotExist(entity))?
		};

		// SAFETY: value is of type component
		unsafe { mut_untyped_to_reflect(value, type_registry, component) }
	}

	/// # Safety
	/// This method does validate that we have access to `R`, but takes `&self`
	/// and as such doesn't check unique access.
	unsafe fn resource_unchecked_mut<R: Resource>(&self) -> Result<Mut<'_, R>, Error> {
		let type_id = TypeId::of::<R>();
		if !self.allows_access_to_resource(type_id) {
			return Err(Error::NoAccessToResource);
		}

		// SAFETY: we have access to `type_id`, caller ensures unique access
		unsafe {
			self
				.world_view
				.world
				.get_resource_mut::<R>()
				.ok_or(Error::ResourceDoesNotExist)
		}
	}
}

impl<'w> RestrictedWorldView<MutableWorldView<'w>> {
	/// Like [`RestrictedWorldView::split_off_resource`], but takes `self` and returns `'w` lifetimes.
	pub fn split_off_resource_typed<R: Resource>(self) -> Option<(Mut<'w, R>, Self)> {
		let type_id = TypeId::of::<R>();
		assert!(self.allows_access_to_resource(type_id));

		// SAFETY: `self` had `R` access, so we have unique access if we remove it from `self`
		let resource = unsafe { self.world_view.world_mut().get_resource_mut::<R>()? };

		let rest = RestrictedWorldView {
			world_view: self.world_view,
			resources: self.resources.without(type_id),
			components: self.components,
		};

		Some((resource, rest))
	}
}

#[derive(Clone)]
enum Allowed<T> {
	// Allowed if included
	AllowList(SmallVec<[T; 2]>),
	// Allowed if not included
	ForbidList(SmallVec<[T; 2]>),
}

impl<T: Clone + PartialEq> Allowed<T> {
	fn allow_just(value: T) -> Allowed<T> {
		Allowed::AllowList(smallvec![value])
	}
	fn allow(values: impl IntoIterator<Item = T>) -> Allowed<T> {
		Allowed::AllowList(values.into_iter().collect())
	}
	fn everything() -> Allowed<T> {
		Allowed::ForbidList(SmallVec::new())
	}
	fn nothing() -> Allowed<T> {
		Allowed::AllowList(SmallVec::new())
	}

	fn allows_access_to(&self, value: T) -> bool {
		match self {
			Allowed::AllowList(list) => list.contains(&value),
			Allowed::ForbidList(list) => !list.contains(&value),
		}
	}

	fn without(&self, value: T) -> Allowed<T> {
		match self {
			Allowed::AllowList(list) => {
				let position = list
					.iter()
					.position(|item| *item == value)
					.expect("called `without` without access");
				let mut new = list.clone();
				new.swap_remove(position);
				Allowed::AllowList(new)
			}
			Allowed::ForbidList(list) => {
				let mut new = list.clone();
				new.push(value);
				Allowed::ForbidList(new)
			}
		}
	}
	fn without_many(&self, values: impl Iterator<Item = T>) -> Allowed<T>
	where
		T: Copy,
	{
		match self {
			Allowed::AllowList(list) => {
				let new = list.clone();
				for value in values {
					let position = list
						.iter()
						.position(|item| *item == value)
						.expect("called `without` without access");
					let mut new = list.clone();
					new.swap_remove(position);
				}
				Allowed::AllowList(new)
			}
			Allowed::ForbidList(list) => {
				let mut new = list.clone();
				new.extend(values);
				Allowed::ForbidList(new)
			}
		}
	}
}

pub enum ReflectBorrow<'a> {
	Mutable(Mut<'a, dyn Reflect>),
	Immutable(&'a dyn Reflect),
}

impl ReflectBorrow<'_> {
	pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
		match self {
			ReflectBorrow::Mutable(value) => value.downcast_mut(),
			ReflectBorrow::Immutable(_) => None,
		}
	}
	pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
		match self {
			ReflectBorrow::Mutable(value) => value.downcast_ref(),
			ReflectBorrow::Immutable(value) => value.downcast_ref(),
		}
	}
	pub fn is_changed(&self) -> bool {
		match self {
			ReflectBorrow::Mutable(value) => value.is_changed(),
			ReflectBorrow::Immutable(_) => false,
		}
	}
}

// SAFETY: MutUntyped is of type with `type_id`
unsafe fn mut_untyped_to_reflect<'a>(
	value: MutUntyped<'a>,
	type_registry: &TypeRegistry,
	type_id: TypeId,
) -> Result<Mut<'a, dyn Reflect>, Error> {
	let registration = type_registry
		.get(type_id)
		.ok_or(Error::NoTypeRegistration)?;
	let reflect_from_ptr = registration
		.data::<ReflectFromPtr>()
		.ok_or(Error::NoTypeData("ReflectFromPtr"))?;

	assert_eq!(reflect_from_ptr.type_id(), type_id);

	let value = value.map_unchanged(|ptr| {
		// SAFETY: ptr is of type type_id as required in safety contract, type_id was checked above
		unsafe { reflect_from_ptr.as_reflect_mut(ptr) }
	});

	Ok(value)
}

// SAFETY: Untyped is of type with `type_id`
unsafe fn ptr_untyped_to_reflect<'a>(
	value: Ptr<'a>,
	type_registry: &TypeRegistry,
	type_id: TypeId,
) -> Result<&'a dyn Reflect, Error> {
	let registration = type_registry
		.get(type_id)
		.ok_or(Error::NoTypeRegistration)?;
	let reflect_from_ptr = registration
		.data::<ReflectFromPtr>()
		.ok_or(Error::NoTypeData("ReflectFromPtr"))?;

	assert_eq!(reflect_from_ptr.type_id(), type_id);

	// SAFETY: ptr is of type type_id as required in safety contract, type_id was checked above
	let value = unsafe { reflect_from_ptr.as_reflect(value) };

	Ok(value)
}

#[cfg(test)]
mod tests {
	use super::{ImmutableWorldView, MutableWorldView, RestrictedWorldView};
	use bevy::{
		prelude::*,
		reflect::{Reflect, TypeRegistry},
	};
	use std::any::TypeId;

	#[derive(Resource)]
	struct A(String);

	#[derive(Resource, Reflect, Default)]
	#[reflect(Resource)]
	struct B(String);

	#[test]
	fn disjoint_resource_access() {
		let mut world = World::new();
		world.insert_resource(A("a".to_string()));
		world.insert_resource(B("b".to_string()));

		let world = RestrictedWorldView::<ImmutableWorldView>::from(&mut world);

		let (a_view, world) = world.split_off_resource(TypeId::of::<A>());
		a_view.resource::<A>().unwrap();
		world.resource::<B>().unwrap();
	}

	#[test]
	fn disjoint_resource_access_by_id() {
		let mut world = World::new();
		world.insert_resource(A("a".to_string()));
		world.insert_resource(B("b".to_string()));

		let world = RestrictedWorldView::<MutableWorldView>::from(&mut world);

		let (mut a_view, mut world) = world.split_off_resource(TypeId::of::<A>());
		let mut a = a_view.resource_mut::<A>().unwrap();

		let mut type_registry = TypeRegistry::empty();
		type_registry.register::<B>();
		let mut b = world
			.resource_reflect_mut_by_id(TypeId::of::<B>(), &type_registry)
			.unwrap();

		a.0.clear();
		b.downcast_mut::<B>().unwrap().0.clear();
	}

	#[test]
	fn get_two_resources_mut() {
		let mut world = World::new();
		world.insert_resource(A("a".to_string()));
		world.insert_resource(B("b".to_string()));

		let mut world = RestrictedWorldView::<MutableWorldView>::from(&mut world);
		let (a, b) = world.two_resources_mut::<A, B>();
		a.unwrap();
		b.unwrap();
	}

	#[test]
	fn invalid_resource_access() {
		let world = World::new();
		let world = RestrictedWorldView::<ImmutableWorldView>::from(&world);

		let (a_view, a_remaining) = world.split_off_resource(TypeId::of::<A>());

		assert!(a_view.allows_access_to_resource(TypeId::of::<A>()));
		assert!(!a_remaining.allows_access_to_resource(TypeId::of::<A>()));
		assert!(!a_view.allows_access_to_resource(TypeId::of::<B>()));
		assert!(a_remaining.allows_access_to_resource(TypeId::of::<B>()));

		let (b_view, b_remaining) = a_remaining.split_off_resource(TypeId::of::<B>());

		assert!(b_view.allows_access_to_resource(TypeId::of::<B>()));
		assert!(!b_remaining.allows_access_to_resource(TypeId::of::<B>()));
	}

	#[derive(Component, Reflect)]
	struct ComponentA(String);

	#[test]
	fn disjoint_component_access() {
		let mut type_registry = TypeRegistry::empty();
		type_registry.register::<ComponentA>();
		type_registry.register::<String>();

		let mut world = World::new();
		world.insert_resource(A("a".to_string()));
		let entity = world.spawn(ComponentA("a".to_string())).id();

		let world = RestrictedWorldView::<MutableWorldView>::from(&mut world);

		let (mut component_view, mut world) =
			world.split_off_component((entity, TypeId::of::<ComponentA>()));
		let mut component = component_view
			.entity_component_reflect_mut(entity, TypeId::of::<ComponentA>(), &type_registry)
			.unwrap();
		let mut resource = world.resource_mut::<A>().unwrap();

		component.downcast_mut::<ComponentA>().unwrap().0.clear();
		resource.0.clear();
	}
}
