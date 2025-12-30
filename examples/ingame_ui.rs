use beditor::prelude::*;
use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((
			EditorPlugin::new(),
			EditorExtensionPlugin::<GameCameraPlugin>::default(),
		))
		.add_systems(Startup, startup)
		.run();
}

#[derive(Default)]
struct GameCameraPlugin;

impl EditorExtension for GameCameraPlugin {
	fn build(&self, ctx: EditorExtensionContext) {
		ctx.register_game_camera::<GameCamera>();
	}
}

#[derive(Component, Reflect, Default, Identifiable)]
#[id("141940fe-15d6-4fc0-a0e3-a199d71f8df4")]
struct GameCamera;

fn startup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	commands.spawn((
		Name::new("Base"),
		Mesh3d(meshes.add(Circle::new(4.0))),
		MeshMaterial3d(materials.add(Color::WHITE)),
		Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
	));
	commands.spawn((
		Name::new("Cube"),
		Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
		MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
		Transform::from_xyz(0.0, 0.5, 0.0),
	));
	commands.spawn((
		Name::new("Light"),
		PointLight {
			shadows_enabled: true,
			..default()
		},
		Transform::from_xyz(4.0, 8.0, 4.0),
	));

	let cam = commands
		.spawn((
			Name::new("Game Camera"),
			GameCamera,
			Camera3d::default(),
			Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
		))
		.id();

	commands
		.spawn((
			UiTargetCamera(cam),
			Node {
				width: vw(100),
				height: vh(100),
				justify_content: JustifyContent::Center,
				align_items: AlignItems::Center,
				..default()
			},
		))
		.with_children(|commands| {
			commands
				.spawn((
					Button,
					Text::new("Click Me"),
					TextColor(Color::linear_rgb(0.03, 0.20, 0.00)),
					BackgroundColor(Color::linear_rgb(0.12, 0.00, 0.76)),
					BorderColor::all(Color::linear_rgb(0.36, 0.09, 0.00)),
					Node {
						width: vw(10),
						height: vh(10),
						..default()
					},
				))
				.observe(|_: On<Pointer<Click>>| {
					info!("Clicked");
				});
		});
}
