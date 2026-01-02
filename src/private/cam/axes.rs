// inspired by https://github.com/maximsnoep/bevy-axes-gizmo

use super::{EDITOR_AXIS_RENDER_LAYER, EditorCamera};
use crate::private::{EditorInternalSingle, UserHidden};
use bevy::{
	camera::visibility::RenderLayers, prelude::*, render::render_resource::TextureFormat,
	ui::FocusPolicy,
};

pub struct AxesGizmoPlugin;

impl Plugin for AxesGizmoPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<AxesGizmoConfig>()
			.init_gizmo_group::<AxesGizmos>()
			.add_observer(on_new_editor_camera)
			.add_systems(Startup, startup)
			.add_systems(PostUpdate, sync.after(TransformSystems::Propagate));
	}
}

/// Plugin for the axes gizmo
#[derive(Resource, Reflect, Clone)]
#[reflect(Resource, Default, Clone)]
pub struct AxesGizmoConfig {
	pub colors: [Color; 3],
	pub length: f32,
}

impl Default for AxesGizmoConfig {
	fn default() -> Self {
		Self {
			colors: [
				Color::linear_rgb(1., 0., 0.),
				Color::linear_rgb(0., 1., 0.),
				Color::linear_rgb(0., 0., 1.),
			],
			length: 100.0,
		}
	}
}

// We can create our own gizmo config group!
#[derive(Default, Reflect, GizmoConfigGroup)]
struct AxesGizmos;

#[derive(Component)]
#[require(UserHidden)]
struct AxesGizmoCamera;

#[derive(Component)]
#[relationship_target(relationship = EditorCameraUi, linked_spawn)]
struct EditorCameraUis(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = EditorCameraUis)]
struct EditorCameraUi(Entity);
fn startup(mut config_store: ResMut<GizmoConfigStore>) {
	let (config, _) = config_store.config_mut::<AxesGizmos>();
	config.render_layers = RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER);
}

fn sync(
	axes_cam_transform: EditorInternalSingle<&GlobalTransform, With<AxesGizmoCamera>>,
	config: Res<AxesGizmoConfig>,
	mut gizmos: Gizmos<AxesGizmos>,
) {
	let start = axes_cam_transform.translation() + axes_cam_transform.forward() * 100.0;
	for (i, axis) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
		gizmos.line(start, start + axis * config.length, config.colors[i]);
	}
}

fn on_new_editor_camera(
	event: On<Add, EditorCamera>,
	mut commands: Commands,
	mut images: ResMut<Assets<Image>>,
) {
	let editor_camera = event.event_target();

	// Create the texture
	let handle = images.add(Image::new_target_texture(
		256,
		256,
		TextureFormat::Bgra8UnormSrgb,
	));

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
		Name::new("Axes Gizmo Camera"),
		Camera3d::default(),
		AxesGizmoCamera,
		Projection::Orthographic(OrthographicProjection::default_3d()),
		Camera {
			target: handle.into(),
			clear_color: ClearColorConfig::Custom(Color::NONE),
			..default()
		},
		RenderLayers::layer(EDITOR_AXIS_RENDER_LAYER),
		ChildOf(editor_camera),
	));
}
