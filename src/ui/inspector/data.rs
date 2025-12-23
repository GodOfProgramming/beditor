use std::{any::Any, fmt::Debug};

use bevy::{
	camera::Camera3dDepthLoadOp,
	light::cluster::ClusterConfig,
	platform::collections::HashMap,
	prelude::*,
	reflect::{self, FromType, GetTypeRegistration, TypeData, TypeInfo, TypeRegistry, VariantInfo},
	render::view::{ColorGradingGlobal, ColorGradingSection},
	time,
};
use nameof::name_of;

use crate::ui::inspector::options::NumberOptions;

#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[non_exhaustive]
pub enum Target {
	Field(usize),
	VariantField {
		variant_index: usize,
		field_index: usize,
	},
}

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
fn insert_options_enum<T: 'static + GetTypeRegistration + reflect::Enum>(
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

pub fn register_type_data(type_registry: &mut TypeRegistry) {
	register_default_options(type_registry);
}

fn register_default_options(type_registry: &mut TypeRegistry) {
	insert_options_struct::<Srgba>(
		type_registry,
		&[
			(name_of!(red in Srgba), &NumberOptions::<f32>::normalized()),
			(
				name_of!(green in Srgba),
				&NumberOptions::<f32>::normalized(),
			),
			(name_of!(blue in Srgba), &NumberOptions::<f32>::normalized()),
			(
				name_of!(alpha in Srgba),
				&NumberOptions::<f32>::normalized(),
			),
		],
	);
	insert_options_struct::<LinearRgba>(
		type_registry,
		&[
			(
				name_of!(red in LinearRgba),
				&NumberOptions::<f32>::positive(),
			),
			(
				name_of!(green in LinearRgba),
				&NumberOptions::<f32>::positive(),
			),
			(
				name_of!(blue in LinearRgba),
				&NumberOptions::<f32>::positive(),
			),
			(
				name_of!(alpha in LinearRgba),
				&NumberOptions::<f32>::positive(),
			),
		],
	);
	insert_options_struct::<Hsla>(
		type_registry,
		&[
			(
				name_of!(hue in Hsla),
				&NumberOptions::<f32>::between(0.0, 360.0),
			),
			(
				name_of!(saturation in Hsla),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(lightness in Hsla),
				&NumberOptions::<f32>::normalized(),
			),
			(name_of!(alpha in Hsla), &NumberOptions::<f32>::normalized()),
		],
	);
	insert_options_struct::<Hsva>(
		type_registry,
		&[
			(
				name_of!(hue in Hsva),
				&NumberOptions::<f32>::between(0.0, 360.0),
			),
			(
				name_of!(saturation in Hsva),
				&NumberOptions::<f32>::normalized(),
			),
			(name_of!(value in Hsva), &NumberOptions::<f32>::normalized()),
			(name_of!(alpha in Hsva), &NumberOptions::<f32>::normalized()),
		],
	);
	insert_options_struct::<Hwba>(
		type_registry,
		&[
			(
				name_of!(hue in Hwba),
				&NumberOptions::<f32>::between(0.0, 360.0),
			),
			(
				name_of!(whiteness in Hwba),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(blackness in Hwba),
				&NumberOptions::<f32>::normalized(),
			),
			(name_of!(alpha in Hwba), &NumberOptions::<f32>::normalized()),
		],
	);
	insert_options_struct::<Laba>(
		type_registry,
		&[
			(
				name_of!(lightness in Laba),
				&NumberOptions::<f32>::between(0.0, 1.5),
			),
			(
				name_of!(a in Laba),
				&NumberOptions::<f32>::between(-1.5, 1.5),
			),
			(
				name_of!(b in Laba),
				&NumberOptions::<f32>::between(-1.5, 1.5),
			),
			(name_of!(alpha in Laba), &NumberOptions::<f32>::normalized()),
		],
	);
	insert_options_struct::<Lcha>(
		type_registry,
		&[
			(
				name_of!(lightness in Lcha),
				&NumberOptions::<f32>::between(0.0, 1.5),
			),
			(
				name_of!(chroma in Lcha),
				&NumberOptions::<f32>::between(0.0, 1.5),
			),
			(
				name_of!(hue in Lcha),
				&NumberOptions::<f32>::between(0.0, 360.0),
			),
			(name_of!(alpha in Lcha), &NumberOptions::<f32>::normalized()),
		],
	);
	insert_options_struct::<Oklaba>(
		type_registry,
		&[
			(
				name_of!(lightness in Oklaba),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(a in Oklaba),
				&NumberOptions::<f32>::between(-1.0, 1.0),
			),
			(
				name_of!(b in Oklaba),
				&NumberOptions::<f32>::between(-1.0, 1.0),
			),
			(
				name_of!(alpha in Oklaba),
				&NumberOptions::<f32>::normalized(),
			),
		],
	);
	insert_options_struct::<Oklcha>(
		type_registry,
		&[
			(
				name_of!(lightness in Oklcha),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(chroma in Oklcha),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(hue in Oklcha),
				&NumberOptions::<f32>::between(0.0, 360.0),
			),
			(
				name_of!(alpha in Oklcha),
				&NumberOptions::<f32>::normalized(),
			),
		],
	);
	insert_options_struct::<Xyza>(
		type_registry,
		&[
			(name_of!(x in Xyza), &NumberOptions::<f32>::normalized()),
			(name_of!(y in Xyza), &NumberOptions::<f32>::normalized()),
			(name_of!(z in Xyza), &NumberOptions::<f32>::normalized()),
			(name_of!(alpha in Xyza), &NumberOptions::<f32>::normalized()),
		],
	);

	insert_options_struct::<ColorGradingSection>(
		type_registry,
		&[
			(
				name_of!(saturation in ColorGradingSection),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(contrast in ColorGradingSection),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(gamma in ColorGradingSection),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(gain in ColorGradingSection),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(lift in ColorGradingSection),
				&NumberOptions::<f32>::default().with_speed(0.01),
			),
		],
	);

	#[rustfmt::skip]
        insert_options_struct::<ColorGradingGlobal>(
            type_registry,
            &[
                (name_of!(exposure in ColorGradingGlobal), &NumberOptions::<f32>::default().with_speed(0.01)),
                (name_of!(temperature in ColorGradingGlobal), &NumberOptions::<f32>::default().with_speed(0.01)),
                (name_of!(tint in ColorGradingGlobal), &NumberOptions::<f32>::default().with_speed(0.01)),
                (name_of!(hue in ColorGradingGlobal), &NumberOptions::<f32>::positive().with_speed(0.01)),
                (name_of!(post_saturation in ColorGradingGlobal), &NumberOptions::<f32>::positive().with_speed(0.01)),
                (name_of!(midtones_range in ColorGradingGlobal), &NumberOptions::<f32>::positive().with_speed(0.01)),
            ],
        );

	insert_options_struct::<AmbientLight>(
		type_registry,
		&[(
			name_of!(brightness in AmbientLight),
			&NumberOptions::<f32>::positive(),
		)],
	);

	insert_options_struct::<PointLight>(
		type_registry,
		&[
			(
				name_of!(intensity in PointLight),
				&NumberOptions::<f32>::positive(),
			),
			(
				name_of!(range in PointLight),
				&NumberOptions::<f32>::positive(),
			),
			(
				name_of!(radius in PointLight),
				&NumberOptions::<f32>::positive(),
			),
		],
	);

	insert_options_struct::<DirectionalLight>(
		type_registry,
		&[(
			name_of!(illuminance in DirectionalLight),
			&NumberOptions::<f32>::positive(),
		)],
	);

	insert_options_struct::<StandardMaterial>(
		type_registry,
		&[
			(
				name_of!(perceptual_roughness in StandardMaterial),
				&NumberOptions::<f32>::between(0.089, 1.0),
			),
			(
				name_of!(metallic in StandardMaterial),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(reflectance in StandardMaterial),
				&NumberOptions::<f32>::normalized(),
			),
			(
				name_of!(depth_bias in StandardMaterial),
				&NumberOptions::<f32>::positive(),
			),
		],
	);

	insert_options_enum::<ClusterConfig>(
		type_registry,
		&[
			(
				macros::name_of_enum_struct!(z_slices in ClusterConfig::FixedZ),
				&NumberOptions::<u32>::at_least(1),
			),
			(
				macros::name_of_enum_struct!(dimensions in ClusterConfig::XYZ),
				&NumberOptions::<UVec3>::at_least(UVec3::ONE),
			),
		],
	);

	insert_options_enum::<Camera3dDepthLoadOp>(
		type_registry,
		&[(
			macros::name_of_enum_tuple!(0 in Camera3dDepthLoadOp::Clear),
			&NumberOptions::<f32>::normalized(),
		)],
	);

	type_registry.register::<time::Virtual>();

	insert_options_struct::<time::Virtual>(
		type_registry,
		&[
			// private fields
			("relative_speed", &NumberOptions::<f64>::positive()),
			("effective_speed", &NumberOptions::<f64>::positive()),
		],
	);
}
