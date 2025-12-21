use crate::{
	ui::{EditorUi, EditorUiHitCaptureNode},
	view::cam::EditorCamera,
};
use bevy::{
	camera::{NormalizedRenderTarget, RenderTarget},
	ecs::system::SystemParam,
	picking::{
		PickingSystems,
		hover::HoverMap,
		pointer::{Location, PointerId, PointerInput},
	},
	prelude::*,
	render::render_resource::Extent3d,
};
use bevy_egui::EguiContexts;
use smallvec::SmallVec;
use uuid::uuid;

#[derive(Default, Component, Reflect)]
pub struct EditorView;

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	editor_camera: ParamSet<
		'w,
		's,
		(
			Option<Single<'w, 's, &'static mut Camera, With<EditorCamera>>>,
			Option<Single<'w, 's, (&'static mut Camera, &'static mut ScreenSpace), With<EditorCamera>>>,
		),
	>,
	contexts: EguiContexts<'w, 's>,
	images: ResMut<'w, Assets<Image>>,
}

impl EditorUi for EditorView {
	const NAME: &str = "Editor View";
	const ID: uuid::Uuid = uuid!("c910a397-a017-4a29-99bc-6282b4b1a214");

	const CAN_CLEAR: bool = false;

	const UNIQUE: bool = true;

	const POPOUT: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, false];

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		app.add_observer(insert_screenspace).add_systems(
			First,
			(
				viewport_picking.in_set(PickingSystems::PostInput),
				clear_viewport_rects,
			)
				.chain(),
		);
	}

	fn spawn(mut params: Self::Params<'_, '_>) -> Self {
		if let Some(mut editor_camera) = params.editor_camera.p0() {
			editor_camera.is_active = true;
		}

		default()
	}

	fn on_despawn(&mut self, mut params: Self::Params<'_, '_>) {
		let Some(mut editor_camera) = params.editor_camera.p0() else {
			return;
		};

		editor_camera.is_active = false;
	}

	fn render(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			mut editor_camera,
			contexts,
			mut images,
		} = params;

		let Some(mut editor_camera) = editor_camera.p1() else {
			ui.label("No camera type selected");
			return;
		};

		let (editor_camera, screen_space) = &mut *editor_camera;

		let RenderTarget::Image(target) = &editor_camera.target else {
			return;
		};

		let Some(tex) = contexts.image_id(target.handle.id()) else {
			return;
		};

		let egui_rect = ui.clip_rect();

		***screen_space = Some(Rect::from_corners(
			Vec2::new(egui_rect.min.x, egui_rect.min.y),
			Vec2::new(egui_rect.max.x, egui_rect.max.y),
		));

		ui.image(egui::load::SizedTexture::new(tex, egui_rect.size()));

		let Some(image) = images.get(target.handle.id()) else {
			return;
		};

		let viewport_size = Rect {
			max: Vec2::new(egui_rect.max.x, egui_rect.max.y),
			min: Vec2::new(egui_rect.min.x, egui_rect.min.y),
		}
		.size()
		.as_uvec2();

		if image.size() == viewport_size {
			return;
		}

		let Some(image) = images.get_mut(target.handle.id()) else {
			return;
		};

		image.resize(Extent3d {
			width: viewport_size.x,
			height: viewport_size.y,
			depth_or_array_layers: 1,
		})
	}
}

#[derive(Component, Default, Deref, DerefMut)]
struct ScreenSpace(Option<Rect>);

fn insert_screenspace(event: On<Add, EditorCamera>, mut commands: Commands) {
	commands
		.entity(event.event_target())
		.insert(ScreenSpace::default());
}

fn clear_viewport_rects(mut q_screen_space: Query<&mut ScreenSpace>) {
	for mut ss in &mut q_screen_space {
		ss.take();
	}
}

fn viewport_picking(
	mut commands: Commands,
	editor_camera: Single<(&Camera, &PointerId, &ScreenSpace), With<EditorCamera>>,
	ui_hit_node: Single<Entity, With<EditorUiHitCaptureNode>>,
	hover_map: Res<HoverMap>,
	mut pointer_inputs: MessageReader<PointerInput>,
) {
	let (editor_camera, editor_camera_pointer_id, screen_space) = *editor_camera;

	let Some(screen_space) = **screen_space else {
		return;
	};

	let Some(target) = editor_camera.target.as_image() else {
		return;
	};

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

	for node_pointer_id in node_pointers {
		for input in inputs
			.iter()
			.filter(|input| input.pointer_id == node_pointer_id)
		{
			let location = Location {
				position: input.location.position - screen_space.min,
				target: NormalizedRenderTarget::Image(target.clone().into()),
			};

			let msg = PointerInput {
				pointer_id: *editor_camera_pointer_id,
				location,
				action: input.action,
			};

			commands.write_message(msg);
		}
	}
}
