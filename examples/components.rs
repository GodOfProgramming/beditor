use beditor::prelude::*;
use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((
			EditorPlugin::new(),
			EditorExtensionPlugin::<GameComponentsExtension>::default(),
		))
		.add_systems(Startup, startup)
		.add_app_systems(Update, (spin, grow))
		.run();
}

#[derive(Default)]
struct GameComponentsExtension;

impl EditorExtension for GameComponentsExtension {
	fn build_editor(&self, ctx: &mut EditorExtensionContext) {
		ctx
			.register_components::<(SpinComponent, GrowthComponent)>()
			.register_components::<(
				ExampleComponent<i8>,
				ExampleComponent<i16>,
				ExampleComponent<i32>,
				ExampleComponent<i64>,
			)>()
			.register_components::<(
				ExampleComponent<u8>,
				ExampleComponent<u16>,
				ExampleComponent<u32>,
				ExampleComponent<u64>,
			)>()
			.register_components::<(ExampleComponent<f32>, ExampleComponent<f64>)>()
			.register_components::<(ExampleComponent<isize>, ExampleComponent<usize>)>();
	}
}

fn startup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	// circular base
	commands.spawn((
		Name::new("Base"),
		Mesh3d(meshes.add(Circle::new(4.0))),
		MeshMaterial3d(materials.add(Color::WHITE)),
		Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
	));
	// cube
	commands.spawn((
		Name::new("Cube"),
		Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
		MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
		Transform::from_xyz(0.0, 0.5, 0.0),
	));
	// light
	commands.spawn((
		Name::new("Light"),
		PointLight {
			shadow_maps_enabled: true,
			..default()
		},
		Transform::from_xyz(4.0, 8.0, 4.0),
	));
}

fn spin(mut q_spins: Query<(&mut Transform, &SpinComponent)>) {
	for (mut transform, spin) in &mut q_spins {
		if spin.velocity != 0.0 {
			transform.rotation *= Quat::from_axis_angle(spin.angle, spin.velocity / 100.0);
		}
	}
}

fn grow(mut q_growths: Query<(&mut Transform, &GrowthComponent)>) {
	for (mut transform, growth) in &mut q_growths {
		if growth.rate != 0.0 {
			transform.scale += growth.dims * growth.rate / 100.0;
		}
	}
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct SpinComponent {
	velocity: f32,
	angle: Vec3,
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct GrowthComponent {
	rate: f32,
	dims: Vec3,
}

#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct ExampleComponent<T>(T)
where
	T: Reflect + Default;
