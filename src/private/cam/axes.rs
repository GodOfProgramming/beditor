// inspired by https://github.com/maximsnoep/bevy-axes-gizmo

use super::{EDITOR_AXIS_RENDER_LAYER, EditorCamera};
use crate::private::{EditorInternalQuery, EditorInternalSingle, UserHidden};
use bevy::{
	camera::visibility::RenderLayers, color::Color, prelude::*,
	render::render_resource::TextureFormat, ui::FocusPolicy,
};

pub struct AxesGizmoPlugin;

impl Plugin for AxesGizmoPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<AxesGizmoConfig>()
			.add_observer(on_new_editor_camera)
			.add_systems(PostUpdate, sync.after(TransformSystems::Propagate));
	}
}

/// Plugin for the axes gizmo
#[derive(Resource, Clone)]
pub struct AxesGizmoConfig {
	pub colors: [Color; 3],
	pub length: f32,
	pub width: f32,
}

impl Default for AxesGizmoConfig {
	fn default() -> Self {
		Self {
			colors: [
				Color::linear_rgb(1., 0., 0.),
				Color::linear_rgb(0., 1., 0.),
				Color::linear_rgb(0., 0., 1.),
			],
			length: 99.,
			width: 2.,
		}
	}
}

#[derive(Component)]
#[require(UserHidden)]
struct AxesGizmoCamera;

#[derive(Component)]
#[require(UserHidden, Visibility, Transform)]
struct AxesGroup;

#[derive(Component)]
#[relationship_target(relationship = EditorCameraUi, linked_spawn)]
struct EditorCameraUis(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = EditorCameraUis)]
struct EditorCameraUi(Entity);

fn sync(
	axes_cam_transform: EditorInternalSingle<&GlobalTransform, With<AxesGizmoCamera>>,
	mut q_axes: EditorInternalQuery<&mut Transform, With<AxesGroup>>,
) {
	for mut axes in &mut q_axes {
		axes.translation = axes_cam_transform.translation() + axes_cam_transform.forward() * 100.0;
	}
}

fn on_new_editor_camera(
	event: On<Add, EditorCamera>,
	mut commands: Commands,
	plugin_config: Res<AxesGizmoConfig>,
	mut images: ResMut<Assets<Image>>,
	mut meshes: ResMut<Assets<bevy::mesh::Mesh>>,
	mut mats: ResMut<Assets<StandardMaterial>>,
) {
	let editor_camera = event.event_target();

	let mesh_axis = meshes.add(Cuboid::new(
		plugin_config.length,
		plugin_config.width,
		plugin_config.width,
	));

	let mut mat_x = StandardMaterial::from_color(plugin_config.colors[0]);
	mat_x.unlit = true;

	let mut mat_y = StandardMaterial::from_color(plugin_config.colors[1]);
	mat_y.unlit = true;

	let mut mat_z = StandardMaterial::from_color(plugin_config.colors[2]);
	mat_z.unlit = true;

	commands.spawn((
		Name::new("View Axes"),
		AxesGroup,
		UserHidden,
		Children::spawn((
			(
				// X AXIS
				Spawn((
					Mesh3d(mesh_axis.clone()),
					MeshMaterial3d(mats.add(mat_x)),
					Transform::from_translation(Vec3::X * (plugin_config.length * 0.5)),
					RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER),
				)),
				// Y AXIS
			),
			Spawn((
				Mesh3d(mesh_axis.clone()),
				MeshMaterial3d(mats.add(mat_y)),
				Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
					.with_translation(Vec3::Y * (plugin_config.length * 0.5)),
				RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER),
			)),
			// Z AXIS
			Spawn((
				Mesh3d(mesh_axis.clone()),
				MeshMaterial3d(mats.add(mat_z)),
				Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
					.with_translation(Vec3::Z * (plugin_config.length * 0.5)),
				RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER),
			)),
		)),
	));

	// Create the texture
	let image = Image::new_target_texture(256, 256, TextureFormat::Bgra8UnormSrgb);
	let handle = images.add(image);

	commands.spawn((
		Name::new("Axis Image"),
		UserHidden,
		Pickable::IGNORE,
		FocusPolicy::Pass,
		UiTargetCamera(editor_camera),
		EditorCameraUi(editor_camera),
		BackgroundColor(Color::NONE),
		Node {
			position_type: PositionType::Absolute,
			left: px(0),
			bottom: px(0),
			width: vmin(20),
			height: vmin(20),
			..default()
		},
		ImageNode {
			image: handle.clone(),
			..default()
		},
	));

	// Spawn the camera
	commands.spawn((
		Camera3d::default(),
		Projection::Orthographic(OrthographicProjection::default_3d()),
		Camera {
			target: handle.into(),
			clear_color: ClearColorConfig::Custom(Color::NONE),
			..default()
		},
		RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER),
		AxesGizmoCamera,
		ChildOf(editor_camera),
	));
}
