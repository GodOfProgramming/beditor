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
