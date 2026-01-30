use super::inspector::InspectorSettings;
use crate::{
	EditorExtension,
	private::{
		EditorInternal,
		cam::{RenderCameras, RenderCamerasSetting},
		util::log::LogLevel,
	},
	storage::ProjectSettings,
	ui::EditorUi,
};
use bevy::{
	camera::{ImageRenderTarget, RenderTarget},
	dev_tools::frame_time_graph::{FrameTimeGraphConfigUniform, FrametimeGraphMaterial},
	diagnostic::DiagnosticsStore,
	ecs::system::SystemParam,
	prelude::*,
	render::{
		render_resource::{Extent3d, TextureFormat},
		storage::ShaderStorageBuffer,
	},
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures, egui};
use common::extensions::egui::ContextExtensions;
use uuid::uuid;

#[derive(Default)]
pub struct DiagnosticsUiExtension;

impl EditorExtension for DiagnosticsUiExtension {
	fn build_editor(&self, ctx: &mut crate::EditorExtensionContext) {
		ctx.register_ui::<DiagnosticsUi>();
	}

	fn build_app(&self, app: &mut App) {
		app.init_resource::<FrameTimeGraph>().add_observer(on_spawn);
	}
}

#[derive(Default, Component, Reflect)]
#[require(GlobalTransform, Visibility)]
pub struct DiagnosticsUi {
	log_level: LogLevel,
}

impl DiagnosticsUi {}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	diagnostics: Res<'w, DiagnosticsStore>,
	render_cameras: ResMut<'w, RenderCameras>,
	inspector_settings: ResMut<'w, InspectorSettings>,
	graph: Res<'w, FrameTimeGraph>,
	images: ResMut<'w, Assets<Image>>,
	project_settings: ProjectSettings<'w, 's>,
}

impl EditorUi for DiagnosticsUi {
	const NAME: &str = "Diagnostics";
	const ID: uuid::Uuid = uuid!("9473f6e1-a595-41e2-8e29-a4f041580fa6");

	const UNIQUE: bool = true;

	type Params<'w, 's> = Params<'w, 's>;

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		Self::default()
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Params {
			diagnostics,
			mut render_cameras,
			mut inspector_settings,
			graph,
			mut images,
			mut project_settings,
		} = params;

		egui::Grid::new("sys-diagnostics").show(ui, |ui| {
			for diagnostic in diagnostics.iter() {
				ui.label(diagnostic.path().as_str());
				if let Some(average) = diagnostic.average() {
					ui.label(format!("{:.2}", average));
				}
				ui.end_row();
			}
		});

		ui.separator();

		let ctx = ui.ctx().clone();
		ctx.inspection_ui(ui);

		ui.separator();

		if ui.checkbox(&mut render_cameras, "Render Cameras").clicked() {
			project_settings
				.set(RenderCamerasSetting, **render_cameras)
				.ok();
		}

		let _ = ui.checkbox(
			&mut inspector_settings.highlight_changes,
			"Highlight Component Changes",
		);

		ui.separator();

		let Some(image) = images.get(graph.image.id()) else {
			ui.label("No Graph Image");
			return;
		};

		let mut ui_size = ui.available_size();
		if ui_size.x <= ui_size.y {
			ui_size.y = ui_size.x;
		} else {
			ui_size.x = ui_size.y;
		}

		ui.image(egui::load::SizedTexture::new(graph.tex, ui_size));

		let image_size = image.size();

		let ui_viewport_size = ui.ctx().to_pixels(ui_size);
		let ui_viewport_size = Vec2::new(ui_viewport_size.x, ui_viewport_size.y).as_uvec2();

		if ui_viewport_size == UVec2::ZERO || image_size == ui_viewport_size {
			return;
		}

		let Some(image) = images.get_mut(graph.image.id()) else {
			ui.label("No Graph Image (mut)");
			return;
		};

		image.resize(Extent3d {
			width: ui_viewport_size.x,
			height: ui_viewport_size.y,
			depth_or_array_layers: 1,
		});
	}
}

#[derive(Resource, Default)]
struct FrameTimeGraph {
	image: Handle<Image>,
	mat: Handle<FrametimeGraphMaterial>,
	tex: egui::TextureId,
}

#[derive(Component)]
struct FrameTimeGraphCamera;

fn on_spawn(
	event: On<Add, DiagnosticsUi>,
	mut commands: Commands,
	mut frame_time_graph_materials: ResMut<Assets<FrametimeGraphMaterial>>,
	mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
	mut images: ResMut<Assets<Image>>,
	mut graph: ResMut<FrameTimeGraph>,
	mut user_textures: ResMut<EguiUserTextures>,
) {
	graph.image = images.add(Image::new_target_texture(
		1,
		1,
		TextureFormat::bevy_default(),
	));

	graph.tex = user_textures.add_image(EguiTextureHandle::Weak(graph.image.id()));

	graph.mat = frame_time_graph_materials.add(FrametimeGraphMaterial {
		values: buffers.add(ShaderStorageBuffer {
			// Initialize with dummy data because the default (`data: None`) will
			// cause a panic in the shader if the frame time graph is constructed
			// with `enabled: false`.
			data: Some(vec![0, 0, 0, 0]),
			..Default::default()
		}),
		config: FrameTimeGraphConfigUniform::new(60.0, 30.0, false),
	});

	let graph_camera = commands
		.spawn((
			Name::new("Frame Graph Camera"),
			EditorInternal,
			FrameTimeGraphCamera,
			Camera2d,
			Camera {
				target: RenderTarget::Image(ImageRenderTarget::from(graph.image.clone())),
				..default()
			},
			ChildOf(event.event_target()),
		))
		.id();

	commands.spawn((
		Name::new("Frame Graph Node"),
		EditorInternal,
		UiTargetCamera(graph_camera),
		Node {
			width: vw(100),
			height: vh(100),
			..Default::default()
		},
		Pickable::IGNORE,
		MaterialNode::from(graph.mat.clone()),
		ChildOf(event.event_target()),
	));
}
