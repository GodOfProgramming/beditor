use std::{borrow::Cow, ffi::OsStr, path::Path};

use crate::util::reflection;
use bevy::{
	platform::collections::HashMap,
	prelude::*,
	reflect::{
		TypeRegistry,
		serde::{TypedReflectDeserializer, TypedReflectSerializer},
	},
};
use derive_new::new;
use ron::ser::PrettyConfig;

type DeserializeFn = fn(bytes: &[u8], type_registry: &TypeRegistry) -> Result<Box<dyn Reflect>>;
type SerializeFn = fn(value: &dyn Reflect, type_registry: &TypeRegistry) -> Result<Vec<u8>>;

#[derive(Resource)]
pub struct SerdeRegistry {
	unknown: Option<SerdeVtable>,
	mapping: HashMap<Cow<'static, OsStr>, SerdeVtable>,
}

impl Default for SerdeRegistry {
	fn default() -> Self {
		Self {
			unknown: default(),
			mapping: default(),
		}
		.with_registration(
			OsStr::new("ron"),
			reflection::serde::ron_serializer,
			reflection::serde::ron_deserializer,
		)
	}
}

impl SerdeRegistry {
	pub fn with_unknown(mut self, ser: SerializeFn, de: DeserializeFn) -> Self {
		self.unknown = Some(SerdeVtable::new(ser, de));
		self
	}

	pub fn add_unknown(&mut self, ser: SerializeFn, de: DeserializeFn) -> &mut Self {
		self.unknown = Some(SerdeVtable::new(ser, de));
		self
	}

	pub fn with_registration(
		mut self,
		extension: impl Into<Cow<'static, OsStr>>,
		ser: SerializeFn,
		de: DeserializeFn,
	) -> Self {
		self.add_registration(extension, ser, de);
		self
	}

	pub fn add_registration(
		&mut self,
		extension: impl Into<Cow<'static, OsStr>>,
		ser: SerializeFn,
		de: DeserializeFn,
	) -> &mut Self {
		self
			.mapping
			.insert(extension.into(), SerdeVtable::new(ser, de));
		self
	}

	pub fn serializer_for(&self, path: &Path) -> Option<SerializeFn> {
		self.vtable_for(path).map(|vtable| vtable.ser)
	}

	pub fn deserializer_for(&self, path: &Path) -> Option<DeserializeFn> {
		self.vtable_for(path).map(|vtable| vtable.de)
	}

	fn vtable_for(&self, path: &Path) -> Option<&SerdeVtable> {
		if let Some(extension) = path.extension() {
			self.mapping.get(extension)
		} else {
			self.unknown.as_ref()
		}
	}
}

#[derive(new)]
struct SerdeVtable {
	ser: SerializeFn,
	de: DeserializeFn,
}

pub fn ron_deserializer(bytes: &[u8], type_registry: &TypeRegistry) -> Result<Box<dyn Reflect>> {
	use serde::de::DeserializeSeed;
	// have to use short names until this is resolved https://github.com/ron-rs/ron/issues/302

	let Some(output) = reflection::ron::newtype_name(bytes) else {
		return Err(String::from("Name of ron struct not found"))?;
	};

	let Some(registration) = type_registry.get_with_short_type_path(&output) else {
		return Err(format!("Type registration of '{output}' not found"))?;
	};

	let reflect_de = TypedReflectDeserializer::new(registration, type_registry);
	let mut ron_de = ron::Deserializer::from_bytes(bytes)?;

	let partial_reflect = reflect_de.deserialize(&mut ron_de)?;

	let Ok(reflect) = partial_reflect.try_into_reflect() else {
		return Err(format!("'{output}' is not Reflect"))?;
	};

	Ok(reflect)
}

pub fn ron_serializer(value: &dyn Reflect, type_registry: &TypeRegistry) -> Result<Vec<u8>> {
	use serde::ser::Serialize;
	let mut buf = String::new();
	let mut ron_ser = ron::Serializer::new(
		&mut buf,
		Some(
			PrettyConfig::default()
				.struct_names(true)
				.escape_strings(true),
		),
	)?;
	let reflect_ser = TypedReflectSerializer::new(value, type_registry);

	reflect_ser.serialize(&mut ron_ser)?;

	Ok(buf.into_bytes())
}
