use std::time::Duration;

use crate::{
	UiManager,
	ui::{EditorUi, TabState, events::ShowUiMessage},
};
use bevy::{
	ecs::system::SystemParam,
	platform::collections::HashSet,
	prelude::*,
	render::view::screenshot::{Screenshot, save_to_disk},
	time::common_conditions::on_timer,
};
use bevy_egui::EguiContexts;
use derive_more::derive::{Deref, DerefMut};
use derive_new::new;
use egui_file_dialog::FileDialog;
use uuid::uuid;

#[derive(Component, Default)]
pub struct ImageViewerUi {
	image: Handle<Image>,
	tex: egui::TextureId,
}

#[derive(SystemParam)]
pub struct Params<'w, 's> {
	commands: Commands<'w, 's>,
	contexts: EguiContexts<'w, 's>,
	images: Res<'w, Assets<Image>>,
	file_dialog: Local<'s, FileDialog>,
}

impl EditorUi for ImageViewerUi {
	const NAME: &str = "Image View";
	const ID: uuid::Uuid = uuid!("5cf9e67a-df8e-4070-a21f-c6301f0ce26f");

	const CAN_CLEAR: bool = true;

	const POPOUT: bool = true;

	const SCROLL_BARS: [bool; 2] = [false, false];

	const REOPEN_ON_STARTUP: bool = false;

	type Params<'w, 's> = Params<'w, 's>;

	fn init(app: &mut App) {
		app.init_resource::<TrackedImages>().add_systems(
			First,
			remove_texture_images.run_if(on_timer(Duration::from_secs(1))),
		);
	}

	fn spawn(_params: Self::Params<'_, '_>) -> Self {
		default()
	}

	fn on_despawn(&mut self, params: Self::Params<'_, '_>) {
		let Self::Params { mut contexts, .. } = params;
		contexts.remove_image(self.image.id());
	}

	fn ui(&mut self, ui: &mut egui::Ui, params: Self::Params<'_, '_>) {
		let Self::Params {
			mut commands,
			contexts,
			images,
			mut file_dialog,
		} = params;

		let Some(image) = images.get(self.image.id()) else {
			ui.label("No Image Selected");
			return;
		};

		file_dialog.update(ui.ctx());

		if let Some(path) = file_dialog.take_picked() {
			commands
				.spawn(Screenshot::image(self.image.clone()))
				.observe(save_to_disk(path));
		}

		let ppp = ui.ctx().pixels_per_point();
		let image_size = image.size();
		let image_size_vec2 = image_size.as_vec2();
		let size_in_points = image_size_vec2 / ppp;
		let size_in_points = if size_in_points.is_finite() {
			size_in_points
		} else {
			image_size_vec2
		};

		let ui_area = ui.clip_rect();

		let texture_rect = egui::Rect::from_center_size(
			ui_area.center(),
			egui::vec2(size_in_points.x, size_in_points.y),
		);

		ui.scope_builder(
			egui::UiBuilder::new()
				.max_rect(texture_rect)
				.layout(egui::Layout::centered_and_justified(
					egui::Direction::TopDown,
				)),
			|ui| {
				ui.image(egui::load::SizedTexture::new(self.tex, texture_rect.size()));
			},
		);
	}

	fn context_menu(
		&mut self,
		ui: &mut egui::Ui,
		params: Self::Params<'_, '_>,
		_surface: egui_dock::SurfaceIndex,
		_node: egui_dock::NodeIndex,
	) {
		let Self::Params {
			mut file_dialog, ..
		} = params;

		if ui.button("Capture").clicked() {
			file_dialog.save_file();
		}
	}
}

#[derive(Resource, Default, Deref, DerefMut)]
struct TrackedImages {
	set: HashSet<AssetId<Image>>,
}

#[derive(new, Deref)]
pub struct OpenImageViewer(pub Handle<Image>);

impl Command for OpenImageViewer {
	fn apply(self, world: &mut World) {
		let mut sys_state = bevy::ecs::system::SystemState::<EguiContexts>::new(world);
		let mut contexts = sys_state.get_mut(world);

		let tex_id = contexts.add_image(bevy_egui::EguiTextureHandle::Weak(self.id()));

		world.resource_mut::<TrackedImages>().insert(self.id());

		let tab = TabState::new::<ImageViewerUi>(world);

		world.entity_mut(tab.entity).insert(ImageViewerUi {
			image: self.0,
			tex: tex_id,
		});

		world.write_message(ShowUiMessage(tab));
	}
}

fn remove_texture_images(
	mut messages: MessageReader<AssetEvent<Image>>,
	mut tracked_images: ResMut<TrackedImages>,
	mut contexts: EguiContexts,
) {
	for msg in messages.read() {
		if let AssetEvent::Removed { id } = msg
			&& tracked_images.remove(id)
		{
			contexts.remove_image(*id);
		}
	}
}
