mod bevy_impls;
mod glam_impls;
mod std_impls;

use crate::inspector::{
	add, add_multiedit, add_single,
	options::{NumberOptions, insert_options_enum, insert_options_struct},
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
use std::{any::TypeId, borrow::Cow, path::PathBuf, time::Instant};

pub fn init_app(app: &mut App) {
	let type_registry = app.world().resource::<AppTypeRegistry>();
	let mut type_registry = type_registry.write();

	register_default_options(&mut type_registry);
	register_std_impls(&mut type_registry);
	register_bevy_impls(&mut type_registry);
	register_glam_impls(&mut type_registry);
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
	add_multiedit::<f32>(type_registry);
	add_multiedit::<f64>(type_registry);
	add_multiedit::<i8>(type_registry);
	add_multiedit::<i16>(type_registry);
	add_multiedit::<i32>(type_registry);
	add_multiedit::<i64>(type_registry);
	add_multiedit::<isize>(type_registry);
	add_multiedit::<u8>(type_registry);
	add_multiedit::<u16>(type_registry);
	add_multiedit::<u32>(type_registry);
	add_multiedit::<u64>(type_registry);
	add_multiedit::<usize>(type_registry);
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
	add_single::<std::ops::RangeInclusive<f32>>(type_registry);

	type_registry.register::<std::ops::RangeInclusive<f64>>();
	add_single::<std::ops::RangeInclusive<f64>>(type_registry);

	add_single::<TypeId>(type_registry);

	add::<std::time::Duration>(type_registry);
	add_single::<Instant>(type_registry);
}

fn register_glam_impls(type_registry: &mut TypeRegistry) {
	add_multiedit::<Vec2>(type_registry);
	add_multiedit::<Vec3>(type_registry);
	add::<Vec3A>(type_registry);
	add::<Vec4>(type_registry);
	add::<UVec2>(type_registry);
	add::<UVec3>(type_registry);
	add::<UVec4>(type_registry);
	add::<IVec2>(type_registry);
	add::<IVec3>(type_registry);
	add::<IVec4>(type_registry);
	add::<DVec2>(type_registry);
	add::<DVec3>(type_registry);
	add::<DVec4>(type_registry);
	add::<BVec2>(type_registry);
	add_single::<BVec3>(type_registry);
	add_single::<BVec4>(type_registry);
	add_single::<Mat2>(type_registry);
	add_single::<Mat3>(type_registry);
	add_single::<Mat3A>(type_registry);
	add_single::<Mat4>(type_registry);
	add_single::<DMat2>(type_registry);
	add_single::<DMat3>(type_registry);
	add_single::<DMat4>(type_registry);
	add::<Quat>(type_registry);
}

fn register_bevy_impls(type_registry: &mut TypeRegistry) {
	add_single::<Entity>(type_registry);

	add::<Color>(type_registry);

	add::<Handle<Mesh>>(type_registry);

	add::<RenderLayers>(type_registry);

	add_single::<Handle<Image>>(type_registry);

	add_single::<GizmoConfigStore>(type_registry);

	add::<uuid::Uuid>(type_registry);

	add::<Name>(type_registry);
}
