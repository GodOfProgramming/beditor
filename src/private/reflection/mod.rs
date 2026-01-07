pub mod inspector;

use std::hash::Hash;

use bevy::{
	prelude::*,
	reflect::{TypeInfo, TypeRegistration},
};
use itertools::Itertools;

pub struct ReflectionExtensionsPlugin;

impl Plugin for ReflectionExtensionsPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<ReflectDefaultCache>()
			.init_resource::<TypeInfoCache>()
			.add_plugins(inspector::EditorInspectorPlugin)
			.add_systems(
				First,
				rebuild_caches.run_if(resource_changed::<AppTypeRegistry>),
			);
	}
}

#[derive(Resource, Default, Deref)]
pub struct ReflectDefaultCache {
	#[deref]
	inner: Vec<&'static TypeInfo>,
}

impl ReflectDefaultCache {
	fn rebuild<'t>(&mut self, type_list: impl Iterator<Item = &'t TypeRegistration>) {
		self.inner = type_list
			.filter_map(|t| t.data::<ReflectDefault>().map(|_| t.type_info()))
			.collect();
	}
}

#[derive(Resource, Default, Deref)]
pub struct TypeInfoCache {
	inner: Vec<CachedTypeInfo>,
}

impl TypeInfoCache {
	fn rebuild<'t>(&mut self, type_list: impl Iterator<Item = &'t TypeRegistration>) {
		self.inner = type_list.map(CachedTypeInfo::from).collect();
	}
}

#[derive(Clone)]
pub struct CachedTypeInfo {
	display: String,
	pub type_info: &'static TypeInfo,
}

impl From<&TypeRegistration> for CachedTypeInfo {
	fn from(value: &TypeRegistration) -> Self {
		let type_info = value.type_info();
		Self {
			display: format!(
				"{} ({})",
				type_info.type_path_table().short_path(),
				type_info.type_path()
			),
			type_info,
		}
	}
}

impl PartialEq for CachedTypeInfo {
	fn eq(&self, other: &Self) -> bool {
		self.type_info.type_id().eq(&other.type_info.type_id())
	}
}

impl Eq for CachedTypeInfo {}

impl Hash for CachedTypeInfo {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.type_info.type_id().hash(state);
	}
}

impl From<CachedTypeInfo> for egui::WidgetText {
	fn from(value: CachedTypeInfo) -> Self {
		Self::Text(value.display.clone())
	}
}

impl From<&CachedTypeInfo> for egui::WidgetText {
	fn from(value: &CachedTypeInfo) -> Self {
		Self::Text(value.display.clone())
	}
}

impl AsRef<str> for CachedTypeInfo {
	fn as_ref(&self) -> &str {
		&self.display
	}
}

fn rebuild_caches(
	app_type_registry: Res<AppTypeRegistry>,
	mut default_cache: ResMut<ReflectDefaultCache>,
	mut display_cache: ResMut<TypeInfoCache>,
) {
	let type_registry = app_type_registry.read();

	let sorted_type_info = type_registry
		.iter()
		.sorted_by(|t1, t2| t1.type_info().type_path().cmp(t2.type_info().type_path()))
		.collect::<Vec<_>>();

	default_cache.rebuild(sorted_type_info.iter().copied());
	display_cache.rebuild(sorted_type_info.iter().copied());
}
