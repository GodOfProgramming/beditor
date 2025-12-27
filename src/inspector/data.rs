mod bevy_impls;
mod glam_impls;
mod std_impls;

use crate::{
	inspector::{
		errors::reflect::no_multiedit,
		options::{NumberOptions, insert_options_enum, insert_options_struct},
		ui::{ImmutableContext, InspectorEguiImpl, InspectorUi, MutableContext, ProjectorReflect},
	},
	util,
};
use bevy::{
	camera::{Camera3dDepthLoadOp, visibility::RenderLayers},
	light::cluster::ClusterConfig,
	math::{DMat2, DMat3, DMat4, DVec2, DVec3, DVec4},
	prelude::*,
	reflect::TypeRegistry,
	render::view::{ColorGradingGlobal, ColorGradingSection},
	time,
};
use nameof::name_of;
use std::{
	any::{Any, TypeId},
	borrow::Cow,
	path::PathBuf,
	time::Instant,
};

pub fn register_type_data(type_registry: &mut TypeRegistry) {
	register_default_options(type_registry);
	register_std_impls(type_registry);
	register_bevy_impls(type_registry);
	register_glam_impls(type_registry);
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

	insert_options_struct::<ColorGradingGlobal>(
		type_registry,
		&[
			(
				name_of!(exposure in ColorGradingGlobal),
				&NumberOptions::<f32>::default().with_speed(0.01),
			),
			(
				name_of!(temperature in ColorGradingGlobal),
				&NumberOptions::<f32>::default().with_speed(0.01),
			),
			(
				name_of!(tint in ColorGradingGlobal),
				&NumberOptions::<f32>::default().with_speed(0.01),
			),
			(
				name_of!(hue in ColorGradingGlobal),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(post_saturation in ColorGradingGlobal),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
			(
				name_of!(midtones_range in ColorGradingGlobal),
				&NumberOptions::<f32>::positive().with_speed(0.01),
			),
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

	insert_options_struct::<time::Virtual>(
		type_registry,
		&[
			// private fields
			("relative_speed", &NumberOptions::<f64>::positive()),
			("effective_speed", &NumberOptions::<f64>::positive()),
		],
	);
}

fn register_std_impls(type_registry: &mut TypeRegistry) {
	add_of_with_many::<f32>(type_registry, std_impls::number_ui_many::<f32>);
	add_of_with_many::<f64>(type_registry, std_impls::number_ui_many::<f64>);
	add_of_with_many::<i8>(type_registry, std_impls::number_ui_many::<i8>);
	add_of_with_many::<i16>(type_registry, std_impls::number_ui_many::<i16>);
	add_of_with_many::<i32>(type_registry, std_impls::number_ui_many::<i32>);
	add_of_with_many::<i64>(type_registry, std_impls::number_ui_many::<i64>);
	add_of_with_many::<isize>(type_registry, std_impls::number_ui_many::<isize>);
	add_of_with_many::<u8>(type_registry, std_impls::number_ui_many::<u8>);
	add_of_with_many::<u16>(type_registry, std_impls::number_ui_many::<u16>);
	add_of_with_many::<u32>(type_registry, std_impls::number_ui_many::<u32>);
	add_of_with_many::<u64>(type_registry, std_impls::number_ui_many::<u64>);
	add_of_with_many::<usize>(type_registry, std_impls::number_ui_many::<usize>);
	add::<bool>(type_registry);
	add::<String>(type_registry);

	type_registry.register::<Cow<str>>();
	add::<Cow<str>>(type_registry);

	type_registry.register::<PathBuf>();
	add::<PathBuf>(type_registry);

	type_registry.register::<std::ops::Range<f32>>();
	add::<std::ops::Range<f32>>(type_registry);

	type_registry.register::<std::ops::Range<f64>>();
	add::<std::ops::Range<f64>>(type_registry);

	type_registry.register::<std::ops::RangeInclusive<f32>>();
	add::<std::ops::RangeInclusive<f32>>(type_registry);

	type_registry.register::<std::ops::RangeInclusive<f64>>();
	add::<std::ops::RangeInclusive<f64>>(type_registry);

	add::<TypeId>(type_registry);

	add::<std::time::Duration>(type_registry);
	add_of_with_many::<Instant>(type_registry, many_unimplemented::<Instant>);
}

fn register_glam_impls(type_registry: &mut TypeRegistry) {
	add_raw::<Vec2>(
		type_registry,
		glam_impls::vec2_ui,
		glam_impls::vec2_ui_readonly,
		glam_impls::vec2_ui_many,
	);
	add_raw::<Vec3>(
		type_registry,
		glam_impls::vec3_ui,
		glam_impls::vec3_ui_readonly,
		glam_impls::vec3_ui_many,
	);
	add_raw::<Vec3A>(
		type_registry,
		glam_impls::vec3a_ui,
		glam_impls::vec3a_ui_readonly,
		glam_impls::vec3a_ui_many,
	);
	add_raw::<Vec4>(
		type_registry,
		glam_impls::vec4_ui,
		glam_impls::vec4_ui_readonly,
		glam_impls::vec4_ui_many,
	);
	add_raw::<UVec2>(
		type_registry,
		glam_impls::uvec2_ui,
		glam_impls::uvec2_ui_readonly,
		glam_impls::uvec2_ui_many,
	);
	add_raw::<UVec3>(
		type_registry,
		glam_impls::uvec3_ui,
		glam_impls::uvec3_ui_readonly,
		glam_impls::uvec3_ui_many,
	);
	add_raw::<UVec4>(
		type_registry,
		glam_impls::uvec4_ui,
		glam_impls::uvec4_ui_readonly,
		glam_impls::uvec4_ui_many,
	);
	add_raw::<IVec2>(
		type_registry,
		glam_impls::ivec2_ui,
		glam_impls::ivec2_ui_readonly,
		glam_impls::ivec2_ui_many,
	);
	add_raw::<IVec3>(
		type_registry,
		glam_impls::ivec3_ui,
		glam_impls::ivec3_ui_readonly,
		glam_impls::ivec3_ui_many,
	);
	add_raw::<IVec4>(
		type_registry,
		glam_impls::ivec4_ui,
		glam_impls::ivec4_ui_readonly,
		glam_impls::ivec4_ui_many,
	);
	add_raw::<DVec2>(
		type_registry,
		glam_impls::dvec2_ui,
		glam_impls::dvec2_ui_readonly,
		glam_impls::dvec2_ui_many,
	);
	add_raw::<DVec3>(
		type_registry,
		glam_impls::dvec3_ui,
		glam_impls::dvec3_ui_readonly,
		glam_impls::dvec3_ui_many,
	);
	add_raw::<DVec4>(
		type_registry,
		glam_impls::dvec4_ui,
		glam_impls::dvec4_ui_readonly,
		glam_impls::dvec4_ui_many,
	);
	add_raw::<BVec2>(
		type_registry,
		glam_impls::bvec2_ui,
		glam_impls::bvec2_ui_readonly,
		many_unimplemented::<BVec2>,
	);
	add_raw::<BVec3>(
		type_registry,
		glam_impls::bvec3_ui,
		glam_impls::bvec3_ui_readonly,
		many_unimplemented::<BVec3>,
	);
	add_raw::<BVec4>(
		type_registry,
		glam_impls::bvec4_ui,
		glam_impls::bvec4_ui_readonly,
		many_unimplemented::<BVec4>,
	);
	add_raw::<Mat2>(
		type_registry,
		glam_impls::mat2_ui,
		glam_impls::mat2_ui_readonly,
		many_unimplemented::<Mat2>,
	);
	add_raw::<Mat3>(
		type_registry,
		glam_impls::mat3_ui,
		glam_impls::mat3_ui_readonly,
		many_unimplemented::<Mat3>,
	);
	add_raw::<Mat3A>(
		type_registry,
		glam_impls::mat3a_ui,
		glam_impls::mat3a_ui_readonly,
		many_unimplemented::<Mat3A>,
	);
	add_raw::<Mat4>(
		type_registry,
		glam_impls::mat4_ui,
		glam_impls::mat4_ui_readonly,
		many_unimplemented::<Mat4>,
	);
	add_raw::<DMat2>(
		type_registry,
		glam_impls::dmat2_ui,
		glam_impls::dmat2_ui_readonly,
		many_unimplemented::<DMat2>,
	);
	add_raw::<DMat3>(
		type_registry,
		glam_impls::dmat3_ui,
		glam_impls::dmat3_ui_readonly,
		many_unimplemented::<DMat3>,
	);
	add_raw::<DMat4>(
		type_registry,
		glam_impls::dmat4_ui,
		glam_impls::dmat4_ui_readonly,
		many_unimplemented::<DMat4>,
	);

	add_raw::<Quat>(
		type_registry,
		glam_impls::quat::quat_ui,
		glam_impls::quat::quat_ui_readonly,
		glam_impls::quat::quat_ui_many,
	);
}

fn register_bevy_impls(type_registry: &mut TypeRegistry) {
	add_of_with_many::<Entity>(type_registry, many_unimplemented::<Entity>);

	add::<Color>(type_registry);

	add_of_with_many::<Handle<Mesh>>(type_registry, many_unimplemented::<Handle<Mesh>>);

	add::<RenderLayers>(type_registry);

	add_of_with_many::<Handle<Image>>(type_registry, many_unimplemented::<Handle<Image>>);

	add::<GizmoConfigStore>(type_registry);

	add::<uuid::Uuid>(type_registry);

	add::<Name>(type_registry);
}

////////////////////////////////////////////////////////////////////////////////

type InspectorEguiImplFn = for<'c> fn(
	&mut dyn Any,
	&mut egui::Ui,
	&dyn Any,
	egui::Id,
	InspectorUi<'_, 'c, MutableContext<'c>>,
) -> bool;

type InspectorEguiImplFnReadonly = for<'c> fn(
	&dyn Any,
	&mut egui::Ui,
	&dyn Any,
	egui::Id,
	InspectorUi<'_, 'c, ImmutableContext<'c>>,
);

type InspectorEguiImplFnMany = for<'c, 'a> fn(
	&mut egui::Ui,
	&dyn Any,
	egui::Id,
	InspectorUi<'_, 'c, MutableContext<'c>>,
	&mut [&mut dyn PartialReflect],
	&dyn ProjectorReflect,
) -> bool;

pub trait InspectorPrimitive: Reflect {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, 'c, MutableContext<'c>>,
	) -> bool;

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, 'c, ImmutableContext<'c>>,
	);
}

fn add<T: InspectorPrimitive + TypePath>(type_registry: &mut TypeRegistry) {
	type_registry.register_type_data::<T, InspectorEguiImpl>();
}

fn add_of_with_many<T: InspectorPrimitive>(
	type_registry: &mut TypeRegistry,
	fn_many: InspectorEguiImplFnMany,
) {
	type_registry
		.get_mut(TypeId::of::<T>())
		.unwrap_or_else(|| panic!("{} not registered", std::any::type_name::<T>()))
		.insert(InspectorEguiImpl::of_with_many::<T>(fn_many));
}

fn add_raw<T: 'static>(
	type_registry: &mut TypeRegistry,
	fn_mut: InspectorEguiImplFn,
	fn_readonly: InspectorEguiImplFnReadonly,
	fn_many: InspectorEguiImplFnMany,
) {
	type_registry
		.get_mut(TypeId::of::<T>())
		.unwrap_or_else(|| panic!("{} not registered", std::any::type_name::<T>()))
		.insert(InspectorEguiImpl::new(fn_mut, fn_readonly, fn_many));
}

pub fn many_unimplemented<T: Any>(
	ui: &mut egui::Ui,
	_options: &dyn Any,
	_id: egui::Id,
	_env: InspectorUi<'_, '_, MutableContext<'_>>,
	_values: &mut [&mut dyn PartialReflect],
	_projector: &dyn ProjectorReflect,
) -> bool {
	no_multiedit(ui, &util::pretty_type_name::<T>());
	false
}
