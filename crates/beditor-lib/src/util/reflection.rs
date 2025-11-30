use crate::util::reflection::serde::SerdeRegistry;
use bevy::{prelude::*, reflect::TypeInfo};

pub struct ReflectionExtensionsPlugin;

impl Plugin for ReflectionExtensionsPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<SerdeRegistry>()
      .init_resource::<ReflectDefaultCache>()
      .add_systems(
        First,
        ReflectDefaultCache::rebuild_cache.run_if(resource_changed::<AppTypeRegistry>),
      );
  }
}

#[derive(Resource, Default, Deref)]
pub struct ReflectDefaultCache(Vec<&'static TypeInfo>);

impl ReflectDefaultCache {
  fn rebuild_cache(
    mut cache: ResMut<ReflectDefaultCache>,
    app_type_registry: Res<AppTypeRegistry>,
  ) {
    let type_registry = app_type_registry.read();

    cache.0 = type_registry
      .iter()
      .filter_map(|t| t.data::<ReflectDefault>().map(|_| t.type_info()))
      .collect();

    cache.0.sort_by(|a, b| a.type_path().cmp(b.type_path()));
  }
}

pub mod serde {
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
}

pub mod ron {
  use serde::Deserializer;

  pub fn newtype_name(bytes: &[u8]) -> Option<String> {
    const PLACEHOLDER: &str = "__☠_PLACEHOLDER_DO_NOT_USE_☠__";

    let mut output = None;

    let wrapper = Wrapper {
      output: &mut output,
      inner: ron::Deserializer::from_bytes(bytes).ok()?,
    };

    let _ = wrapper.deserialize_newtype_struct(PLACEHOLDER, ExtractVisitor);

    output
  }

  struct ExtractVisitor;

  impl<'de> serde::de::Visitor<'de> for ExtractVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
      write!(formatter, "struct type")
    }

    fn visit_newtype_struct<D>(self, _: D) -> Result<Self::Value, D::Error>
    where
      D: serde::Deserializer<'de>,
    {
      Err(serde::de::Error::custom("ABORT"))
    }
  }

  struct Wrapper<'de, 'o> {
    inner: ron::Deserializer<'de>,
    output: &'o mut Option<String>,
  }

  impl<'de, 'o> Deserializer<'de> for Wrapper<'de, 'o> {
    type Error = ron::de::Error;

    fn deserialize_any<V>(mut self, visitor: V) -> std::result::Result<V::Value, Self::Error>
    where
      V: serde::de::Visitor<'de>,
    {
      self.inner.deserialize_any(visitor)
    }

    fn deserialize_newtype_struct<V>(
      mut self,
      name: &'static str,
      visitor: V,
    ) -> std::result::Result<V::Value, Self::Error>
    where
      V: serde::de::Visitor<'de>,
    {
      self
        .inner
        .deserialize_newtype_struct(name, visitor)
        .inspect_err(|err| {
          if let ron::de::Error::ExpectedDifferentStructName { found, .. } = err {
            *self.output = Some(found.clone());
          }
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf option unit unit_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
  }
}
