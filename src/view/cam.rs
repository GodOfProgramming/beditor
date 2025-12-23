use super::UP;
use crate::{
	ui::EditorUiHitCaptureNode,
	util::storage::{ActiveEditorCameraSetting, ProjectSettings, RenderCamerasSetting},
};
use bevy::{
	camera::{ImageRenderTarget, NormalizedRenderTarget, RenderTarget},
	color::palettes::tailwind,
	picking::{
		PickingSystems,
		hover::HoverMap,
		pointer::{Location, PointerId, PointerInput},
	},
	prelude::*,
	render::render_resource::TextureFormat,
	ui::FocusPolicy,
};
use bevy_axes_gizmo::{AxesGizmoSyncCamera, AxesGizmoTexture};
use derive_more::derive::Deref;
use derive_new::new;
use macros::Identifiable;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use transform_gizmo_bevy::{GizmoCamera, GizmoOptions};
use uuid::Uuid;

pub struct EditorCamPlugin;

impl Plugin for EditorCamPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<RenderCameras>()
			.init_resource::<GameCameraColor>()
			.add_observer(PointCameraEvent::handle)
			.add_observer(MoveCameraEvent::handle)
			.add_observer(SyncRenderCamerasEvent::handle)
			.add_observer(on_manage_camera)
			.add_observer(on_unmanage_camera)
			.add_observer(on_spawn_editor_camera)
			.add_systems(Startup, retrieve_show_cameras_value)
			.add_systems(PostStartup, init_camera)
			.add_systems(OnEnter(ActiveEditorCamera::None), despawn_editor_cameras)
			.add_systems(
				First,
				(
					(
						EditorManagedCamera::viewport_picking.in_set(PickingSystems::PostInput),
						EditorManagedCamera::sync_gizmos,
					),
					EditorManagedCamera::on_frame_end,
				)
					.chain(),
			)
			.add_systems(
				FixedUpdate,
				track_editor_camera_changes.run_if(state_changed::<ActiveEditorCamera>),
			);
	}
}

fn init_camera(
	mut settings: ProjectSettings,
	mut next_state: ResMut<NextState<ActiveEditorCamera>>,
) {
	let state = settings.get_or_default::<ActiveEditorCameraSetting>();
	next_state.set(state);
}

fn on_manage_camera(
	event: On<Add, EditorManagedCamera>,
	mut commands: Commands,
	mut contexts: bevy_egui::EguiContexts,
	mut images: ResMut<Assets<Image>>,
) {
	let image = Image::new_target_texture(1, 1, TextureFormat::Rgba32Float);
	let image_handle = images.add(image);

	contexts.add_image(bevy_egui::EguiTextureHandle::Weak(image_handle.id()));

	commands.entity(event.event_target()).insert(Camera {
		order: isize::MIN,
		target: RenderTarget::Image(ImageRenderTarget::from(image_handle)),
		..default()
	});
}

fn on_unmanage_camera(
	event: On<Remove, EditorManagedCamera>,
	q_cameras: Query<&Camera>,
	mut contexts: bevy_egui::EguiContexts,
) {
	if let Ok(camera) = q_cameras.get(event.event_target())
		&& let RenderTarget::Image(image) = &camera.target
	{
		contexts.remove_image(image.handle.id());
	}
}

fn on_spawn_editor_camera(
	event: On<Add, EditorCamera>,
	mut commands: Commands,
	axes_gizmo_image: Res<AxesGizmoTexture>,
) {
	commands
		.entity(event.event_target())
		.observe(|_: On<Pointer<Click>>| {
			info!("Clicked");
		});
	commands.spawn((
		Name::new("Axis Image"),
		Pickable::IGNORE,
		FocusPolicy::Pass,
		UiTargetCamera(event.event_target()),
		Node {
			position_type: PositionType::Absolute,
			left: px(0),
			bottom: px(0),
			width: vw(20),
			height: vh(20),
			..default()
		},
		BackgroundColor(Color::NONE),
		EditorCameraUi(event.event_target()),
		Children::spawn(Spawn((
			Pickable::IGNORE,
			FocusPolicy::Pass,
			ImageNode {
				image: axes_gizmo_image.0.clone(),
				..default()
			},
		))),
	));
}

fn despawn_editor_cameras(mut commands: Commands, q_cams: Query<Entity, With<EditorCamera>>) {
	info!("Despawning all editor cameras");
	for entity in &q_cams {
		commands.entity(entity).despawn();
	}
}

#[derive(Default, Component, Reflect, Identifiable)]
#[require(
  MeshPickingCamera,
  EditorManagedCamera,
  GizmoCamera = GizmoCamera,
  AxesGizmoSyncCamera = AxesGizmoSyncCamera,
)]
#[id("00000000-0000-0000-0000-000000000000")]
pub struct EditorCamera;

#[derive(Component)]
#[relationship_target(relationship = EditorCameraUi, linked_spawn)]
struct EditorCameraUis(Vec<Entity>);

#[derive(Component)]
#[relationship(relationship_target = EditorCameraUis)]
struct EditorCameraUi(Entity);

#[derive(Component, Default)]
#[require(
  Camera,
  PointerId = PointerId::Custom(Uuid::new_v4()),
)]
pub struct EditorManagedCamera {
	context_menu_opened: bool,
	hovered: bool,
	viewport_rect: Option<Rect>,
	last_viewport: Option<Rect>,
	ignore_size_mismatch: bool,
}

impl EditorManagedCamera {
	pub fn set_ctx_menu_open(&mut self, open: bool) {
		self.context_menu_opened = open;
	}

	pub fn set_hovered(&mut self, hovered: bool) {
		self.hovered = hovered;
	}

	pub fn viewport(&self) -> Option<Rect> {
		self.last_viewport
	}

	pub fn set_viewport(&mut self, rect: Rect) {
		self.viewport_rect = Some(rect);
	}

	pub fn should_sync_to_viewport(&self) -> bool {
		!self.ignore_size_mismatch
	}

	pub fn ignore_viewport_size(&mut self) {
		self.ignore_size_mismatch = true;
	}

	pub fn sync_viewport_size(&mut self) {
		self.ignore_size_mismatch = false;
	}

	fn viewport_picking(
		mut commands: Commands,
		q_managed_cameras: Query<(&Camera, &PointerId, &Self)>,
		ui_hit_node: Single<Entity, With<EditorUiHitCaptureNode>>,
		hover_map: Res<HoverMap>,
		mut pointer_inputs: MessageReader<PointerInput>,
	) {
		let node_pointers = hover_map.iter().flat_map(|(pointer_id, hits)| {
			hits.keys().filter_map(|entity| {
				if *entity == *ui_hit_node {
					Some(*pointer_id)
				} else {
					None
				}
			})
		});

		let inputs = pointer_inputs.read().collect::<SmallVec<[_; 4]>>();

		let filtered_inputs = node_pointers
			.flat_map(|node_pointer_id| {
				inputs
					.iter()
					.filter(move |input| input.pointer_id == node_pointer_id)
			})
			.collect::<SmallVec<[_; 2]>>();

		let iter = q_managed_cameras
			.iter()
			.filter_map(|(camera, managed_camera_pointer_id, managed_camera)| {
				if !managed_camera.hovered || managed_camera.context_menu_opened {
					None
				} else {
					managed_camera
						.viewport_rect
						.zip(camera.target.as_image())
						.map(|(viewport_rect, target)| (target, viewport_rect, managed_camera_pointer_id))
				}
			})
			.flat_map(|(target, viewport_rect, managed_camera_pointer_id)| {
				filtered_inputs
					.iter()
					.map(move |input| (target, viewport_rect, managed_camera_pointer_id, input))
			});

		for (target, viewport_rect, managed_camera_pointer_id, input) in iter {
			let location = Location {
				position: input.location.position - viewport_rect.min,
				target: NormalizedRenderTarget::Image(target.clone().into()),
			};

			let msg = PointerInput {
				pointer_id: *managed_camera_pointer_id,
				location,
				action: input.action,
			};

			commands.write_message(msg);
		}
	}

	fn sync_gizmos(
		editor_camera: Single<&Self, With<EditorCamera>>,
		mut gizmos_options: ResMut<GizmoOptions>,
	) {
		gizmos_options.viewport_rect = editor_camera.viewport_rect;
	}

	fn on_frame_end(mut q_managed_cameras: Query<&mut Self>) {
		for mut cam in &mut q_managed_cameras {
			cam.last_viewport = cam.viewport_rect.take();
			cam.set_ctx_menu_open(false);
			cam.hovered = false;
		}
	}
}

#[derive(
	Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default, Serialize, Deserialize, Reflect,
)]
pub enum ActiveEditorCamera {
	#[default]
	None,
	Cam2D,
	Cam3D,
}

#[derive(Resource, Reflect, Deref)]
#[reflect(Resource, Default)]
pub struct GameCameraColor(Color);

impl Default for GameCameraColor {
	fn default() -> Self {
		Self(tailwind::GREEN_700.into())
	}
}

#[derive(Resource, Reflect, Default, Deref, DerefMut)]
#[reflect(Resource, Default)]
pub struct RenderCameras(bool);

fn track_editor_camera_changes(
	cam_state: Res<State<ActiveEditorCamera>>,
	mut settings: ProjectSettings,
) -> Result {
	settings.set::<ActiveEditorCameraSetting>(**cam_state)
}

#[derive(new, Event)]
pub struct MoveCameraEvent(Vec3);

impl MoveCameraEvent {
	fn handle(event: On<Self>, mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>) {
		for mut cam in &mut q_cam_transforms {
			cam.translation = event.0;
		}
	}
}

#[derive(new, Event)]
pub struct PointCameraEvent(Vec3);

impl PointCameraEvent {
	fn handle(event: On<Self>, mut q_cam_transforms: Query<&mut Transform, With<EditorCamera>>) {
		for mut cam in &mut q_cam_transforms {
			cam.look_at(event.0, UP);
		}
	}
}

#[derive(Event)]
pub struct SyncRenderCamerasEvent;

impl SyncRenderCamerasEvent {
	fn handle(
		_: On<Self>,
		render_cameras: Res<RenderCameras>,
		mut settings: ProjectSettings,
	) -> Result {
		settings.set::<RenderCamerasSetting>(**render_cameras)?;
		Ok(())
	}
}

fn retrieve_show_cameras_value(
	mut render_cameras: ResMut<RenderCameras>,
	mut settings: ProjectSettings,
) {
	**render_cameras = settings.get_or_default::<RenderCamerasSetting>();
}

pub fn should_show_cameras(render_cameras: Res<RenderCameras>) -> bool {
	**render_cameras
}

#[allow(clippy::type_complexity)]
pub fn render_2d_cameras<C: Component>(
	mut gizmos: Gizmos,
	q_cam: Query<(&Transform, &Projection), (With<Camera2d>, With<C>)>,
	cam_color: Res<GameCameraColor>,
) {
	for (transform, projection) in &q_cam {
		if let Projection::Orthographic(ortho) = projection {
			let rect_pos = transform.translation;
			gizmos.rect(rect_pos, ortho.area.max - ortho.area.min, **cam_color);
		}
	}
}

#[allow(clippy::type_complexity)]
pub fn render_3d_cameras<C: Component>(
	mut gizmos: Gizmos,
	q_cam: Query<(&Transform, &Projection), (With<Camera3d>, With<C>)>,
	cam_color: Res<GameCameraColor>,
) {
	for (transform, projection) in &q_cam {
		match projection {
			Projection::Perspective(perspective) => {
				render_3d_camera(
					*transform,
					perspective.aspect_ratio,
					&mut gizmos,
					&cam_color,
				);
			}
			Projection::Orthographic(orthographic) => {
				render_3d_camera(*transform, orthographic.scale, &mut gizmos, &cam_color);
			}
			_ => (),
		}
	}
}

fn render_3d_camera(
	transform: Transform,
	scaler: f32,
	gizmos: &mut Gizmos,
	cam_color: &GameCameraColor,
) {
	gizmos.cuboid(transform, **cam_color);

	let forward = transform.forward().as_vec3();

	let rect_pos = transform.translation + forward;
	let rect_iso = Isometry3d::new(rect_pos, transform.rotation);
	let rect_dim = Vec2::new(scaler, 1.0);

	gizmos.rect(rect_iso, rect_dim, **cam_color);

	let start = transform.translation + forward * transform.scale / 2.0;

	let rect_corners = [
		rect_dim,
		-rect_dim,
		rect_dim.with_x(-rect_dim.x),
		rect_dim.with_y(-rect_dim.y),
	]
	.map(|corner| Vec3::from((corner / 2.0, 0.0)))
	.map(|corner| rect_iso * corner);

	for corner in rect_corners {
		gizmos.line(start, corner, **cam_color);
	}
}
