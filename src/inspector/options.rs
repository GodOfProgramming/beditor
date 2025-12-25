use bevy::{
	platform::collections::HashMap,
	prelude::*,
	reflect::{self, FromType, GetTypeRegistration, TypeData, TypeInfo, TypeRegistry, VariantInfo},
};
use std::{any::Any, collections::VecDeque, fmt::Debug};

#[derive(Default)]
pub struct InspectorOptions {
	options: HashMap<Target, Box<dyn TypeData>>,
}

impl Debug for InspectorOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut options = f.debug_struct("InspectorOptions");
		for entry in self.options.keys() {
			options.field(&format!("{entry:?}"), &"..");
		}
		options.finish()
	}
}

impl Clone for InspectorOptions {
	fn clone(&self) -> Self {
		Self {
			options: self
				.options
				.iter()
				.map(|(target, data)| (*target, TypeData::clone_type_data(&**data)))
				.collect(),
		}
	}
}
impl InspectorOptions {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn insert<T: TypeData>(&mut self, path: Target, options: T) {
		self.options.insert(path, Box::new(options));
	}
	pub fn insert_boxed(&mut self, path: Target, options: Box<dyn TypeData>) {
		self.options.insert(path, options);
	}
	pub fn get(&self, path: Target) -> Option<&dyn Any> {
		self.options.get(&path).map(|value| value.as_any())
	}

	pub fn iter(&self) -> impl Iterator<Item = (Target, &dyn TypeData)> + '_ {
		self.options.iter().map(|(path, data)| (*path, &**data))
	}
}

#[derive(Clone)]
pub struct ReflectInspectorOptions(pub InspectorOptions);

impl<T> FromType<T> for ReflectInspectorOptions
where
	InspectorOptions: FromType<T>,
{
	fn from_type() -> Self {
		ReflectInspectorOptions(InspectorOptions::from_type())
	}
}

pub trait InspectorOptionsType {
	type DeriveOptions: Default;
	type Options: TypeData + Clone;
	fn options_from_derive(options: Self::DeriveOptions) -> Self::Options;
}

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Target {
	Field(usize),
	VariantField {
		variant_index: usize,
		field_index: usize,
	},
}

pub fn insert_options_struct<T: 'static + GetTypeRegistration + reflect::Struct>(
	type_registry: &mut TypeRegistry,
	fields: &[(&'static str, &dyn TypeData)],
) {
	type_registry.register::<T>();

	let Some(registration) = type_registry.get_mut(std::any::TypeId::of::<T>()) else {
		unreachable!("Just registered the type");
	};

	if registration.data::<ReflectInspectorOptions>().is_none() {
		let mut options = InspectorOptions::new();

		for (field, data) in fields {
			let info = match registration.type_info() {
				TypeInfo::Struct(info) => info,
				_ => unreachable!("Struct reflect restriction"),
			};

			let Some(field_index) = info.index_of(field) else {
				let name = T::get_type_registration().type_info().type_path();
				panic!("Field '{field}' does not exist on type {name}");
			};

			options.insert_boxed(Target::Field(field_index), TypeData::clone_type_data(*data));
		}

		registration.insert(ReflectInspectorOptions(options));
	}
}

pub fn insert_options_enum<T: 'static + GetTypeRegistration + reflect::Enum>(
	type_registry: &mut TypeRegistry,
	fields: &[((&'static str, &'static str), &dyn TypeData)],
) {
	type_registry.register::<T>();

	let Some(registration) = type_registry.get_mut(std::any::TypeId::of::<T>()) else {
		unreachable!("Just registered the type");
	};

	if registration.data::<ReflectInspectorOptions>().is_none() {
		let mut options = InspectorOptions::new();
		for ((variant, field), data) in fields {
			let info = match registration.type_info() {
				TypeInfo::Enum(info) => info,
				_ => unreachable!("Enum reflect restriction"),
			};
			let variant_index = info.index_of(variant).unwrap();
			let field_index = match info.variant_at(variant_index).unwrap() {
				VariantInfo::Struct(s) => {
					let Some(i) = s.index_of(field) else {
						let name = T::get_type_registration().type_info().type_path();
						panic!("Could not find field '{field}' on type {name}::{variant}");
					};
					i
				}
				VariantInfo::Tuple(_) => {
					let Ok(i) = field.parse() else {
						let name = T::get_type_registration().type_info().type_path();
						panic!("Could not find field '{field}' on type {name}::{variant}");
					};
					i
				}
				VariantInfo::Unit(_) => {
					let name = T::get_type_registration().type_info().type_path();
					panic!("Tried to access field '{field}' on unit type {name}::{variant}");
				}
			};
			options.insert_boxed(
				Target::VariantField {
					variant_index,
					field_index,
				},
				TypeData::clone_type_data(*data),
			);
		}
		registration.insert(ReflectInspectorOptions(options));
	}
}

#[derive(Clone)]
#[non_exhaustive]
pub struct NumberOptions<T> {
	pub min: Option<T>,
	pub max: Option<T>,
	pub speed: f32,
	pub prefix: String,
	pub suffix: String,
	pub display: NumberDisplay,
}

impl<T> Default for NumberOptions<T> {
	fn default() -> Self {
		Self {
			min: None,
			max: None,
			speed: 0.0,
			prefix: String::new(),
			suffix: String::new(),
			display: NumberDisplay::default(),
		}
	}
}

#[derive(Clone, Copy, Default)]
#[non_exhaustive]
pub enum NumberDisplay {
	#[default]
	Drag,
	Slider,
}

impl<T> NumberOptions<T> {
	pub fn between(min: T, max: T) -> NumberOptions<T> {
		NumberOptions {
			min: Some(min),
			max: Some(max),
			speed: 0.0,
			prefix: String::new(),
			suffix: String::new(),
			display: NumberDisplay::default(),
		}
	}
	pub fn at_least(min: T) -> NumberOptions<T> {
		NumberOptions {
			min: Some(min),
			max: None,
			speed: 0.0,
			prefix: String::new(),
			suffix: String::new(),
			display: NumberDisplay::default(),
		}
	}

	pub fn with_speed(self, speed: f32) -> NumberOptions<T> {
		NumberOptions { speed, ..self }
	}

	pub fn map<U>(&self, f: impl Fn(&T) -> U) -> NumberOptions<U> {
		NumberOptions {
            #[allow(clippy::redundant_closure)] // false positive
            min: self.min.as_ref().map(|min| f(min)),
            max: self.max.as_ref().map(f),
            speed: self.speed,
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            display: NumberDisplay::default(),
        }
	}
}
impl<T: egui::emath::Numeric> NumberOptions<T> {
	pub fn positive() -> NumberOptions<T> {
		NumberOptions {
			min: Some(T::from_f64(0.0)),
			max: None,
			speed: 0.0,
			prefix: String::new(),
			suffix: String::new(),
			display: NumberDisplay::default(),
		}
	}

	pub fn normalized() -> Self {
		NumberOptions {
			min: Some(T::from_f64(0.0)),
			max: Some(T::from_f64(1.0)),
			speed: 0.01,
			prefix: String::new(),
			suffix: String::new(),
			display: NumberDisplay::default(),
		}
	}
}

#[non_exhaustive]
pub struct RangeOptions<T: InspectorOptionsType> {
	pub start: T::Options,
	pub end: T::Options,
}

impl<T: InspectorOptionsType> Clone for RangeOptions<T> {
	fn clone(&self) -> Self {
		Self {
			start: self.start.clone(),
			end: self.end.clone(),
		}
	}
}

impl<T: InspectorOptionsType> Default for RangeOptions<T> {
	fn default() -> Self {
		Self {
			start: T::options_from_derive(T::DeriveOptions::default()),
			end: T::options_from_derive(T::DeriveOptions::default()),
		}
	}
}

impl<T: InspectorOptionsType + 'static> InspectorOptionsType for std::ops::Range<T> {
	type DeriveOptions = RangeOptions<T>;
	type Options = RangeOptions<T>;

	fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
		options
	}
}
impl<T: InspectorOptionsType + 'static> InspectorOptionsType for std::ops::RangeInclusive<T> {
	type DeriveOptions = RangeOptions<T>;
	type Options = RangeOptions<T>;

	fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
		options
	}
}

#[derive(Default, Clone)]
#[non_exhaustive]
pub struct QuatOptions {
	pub display: QuatDisplay,
}

#[derive(Copy, Clone, Default)]
pub enum QuatDisplay {
	Raw,
	#[default]
	Euler,
	YawPitchRoll,
	AxisAngle,
}

#[derive(Clone)]
#[non_exhaustive]
pub struct EntityOptions {
	pub display: EntityDisplay,
	pub despawnable: bool,
	pub highlight_changes: bool,
}

impl Default for EntityOptions {
	fn default() -> Self {
		Self {
			display: EntityDisplay::default(),
			despawnable: true,
			highlight_changes: false,
		}
	}
}

#[derive(Copy, Clone, Default)]
#[non_exhaustive]
pub enum EntityDisplay {
	Id,
	#[default]
	Components,
}

impl<T: InspectorOptionsType> InspectorOptionsType for Option<T> {
	type DeriveOptions = T::DeriveOptions;
	type Options = InspectorOptions;

	fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
		let inner_options = T::options_from_derive(options);

		let mut inspector_options = InspectorOptions::new();
		inspector_options.insert(
			Target::VariantField {
				variant_index: 1, // Some
				field_index: 0,
			},
			inner_options,
		);

		inspector_options
	}
}

impl<T: InspectorOptionsType, const N: usize> InspectorOptionsType for [T; N] {
	type DeriveOptions = T::DeriveOptions;
	type Options = T::Options;

	fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
		T::options_from_derive(options)
	}
}

macro_rules! impl_options {
	($ty:ty => $options:ty) => {
		impl InspectorOptionsType for $ty {
			type DeriveOptions = $options;
			type Options = $options;

			fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
				options
			}
		}
	};
}

impl_options!(f32 => NumberOptions<f32>);
impl_options!(f64 => NumberOptions<f64>);
impl_options!(i8 => NumberOptions<i8>);
impl_options!(i16 => NumberOptions<i16>);
impl_options!(i32 => NumberOptions<i32>);
impl_options!(i64 => NumberOptions<i64>);
impl_options!(i128 => NumberOptions<i128>);
impl_options!(isize => NumberOptions<isize>);
impl_options!(u8 => NumberOptions<u8>);
impl_options!(u16 => NumberOptions<u16>);
impl_options!(u32 => NumberOptions<u32>);
impl_options!(u64 => NumberOptions<u64>);
impl_options!(u128 => NumberOptions<u128>);
impl_options!(usize => NumberOptions<usize>);
impl_options!(Quat => QuatOptions);
impl_options!(Entity => EntityOptions);

macro_rules! impl_options_defer_generic {
	($name:ident < $generic:ident >) => {
		impl<$generic: InspectorOptionsType> InspectorOptionsType for $name<$generic> {
			type DeriveOptions = <$generic as InspectorOptionsType>::DeriveOptions;
			type Options = <$generic as InspectorOptionsType>::Options;

			fn options_from_derive(options: Self::DeriveOptions) -> Self::Options {
				$generic::options_from_derive(options)
			}
		}
	};
}

impl_options_defer_generic!(Vec<T>);
impl_options_defer_generic!(VecDeque<T>);
