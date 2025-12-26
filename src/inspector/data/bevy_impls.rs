mod image_texture_conversion;

use super::InspectorPrimitive;
use crate::{
	inspector::{
		errors::{dead_asset_handle, no_world_in_context},
		options::{EntityDisplay, EntityOptions},
		ui::{Context, InspectorUi},
	},
	ui::widgets,
	util::{self, pretty_type_name, world::RestrictedWorldView},
};
use bevy::{
	camera::visibility::RenderLayers,
	gizmos::config::GizmoConfigStore,
	mesh::Indices,
	platform::collections::{HashMap, HashSet, hash_map},
	prelude::*,
	reflect::DynamicTypePath,
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use egui::load::SizedTexture;
use parking_lot::Mutex;
use std::{any::Any, sync::LazyLock};

static SCALED_DOWN_TEXTURES: LazyLock<Mutex<ScaledDownTextures>> = LazyLock::new(Default::default);

impl InspectorPrimitive for uuid::Uuid {
	fn ui(&mut self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) -> bool {
		ui.label(self.to_string());
		false
	}
	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) {
		ui.label(self.to_string());
	}
}

impl InspectorPrimitive for Entity {
	fn ui(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		mut env: InspectorUi<'_, '_>,
	) -> bool {
		let entity = *self;

		let options = options
			.downcast_ref::<EntityOptions>()
			.cloned()
			.unwrap_or_default();

		match options.display {
			EntityDisplay::Id => {
				ui.label(format!("{entity:?}"));
			}
			EntityDisplay::Components => {
				let Some(ctx) = &mut env.context else {
					no_world_in_context(ui, "Entity");
					return false;
				};

				let entity_name =
					util::entity::guess_entity_name_restricted(unsafe { ctx.world.world().world() }, entity);

				egui::CollapsingHeader::new(entity_name)
					.id_salt(id)
					.show(ui, |ui| {
						crate::inspector::ui::components::ui_for_entity_components(
							ctx,
							entity,
							ui,
							id,
							env.type_registry,
							options.highlight_changes,
						);
						if options.despawnable
							&& ctx.world.contains_entity(entity)
							&& util::egui::label_button(ui, "✖ Despawn", egui::Color32::RED)
						{
							ctx.queue.push(move |world: &mut World| {
								world.entity_mut(entity).despawn();
							});
						}
					});
			}
		}
		false
	}

	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) {
		ui.label(format!("{self:?}"));
	}
}

impl InspectorPrimitive for Handle<Mesh> {
	fn ui(&mut self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, env: InspectorUi<'_, '_>) -> bool {
		let handle = &*self;
		let Some(Context { world, .. }) = env.context else {
			no_world_in_context(ui, "Handle<Mesh>");
			return false;
		};
		let mut meshes = match world.get_resource_mut::<Assets<Mesh>>() {
			Ok(meshes) => meshes,
			Err(e) => {
				e.ui(ui, "Assets<Mesh>");
				return false;
			}
		};
		let Some(mesh) = meshes.get_mut(handle) else {
			dead_asset_handle(ui, handle.id().untyped());
			return false;
		};

		mesh_ui_inner(mesh, ui);

		ui.add_enabled_ui(mesh.indices().is_some(), |ui| {
			if ui.button("Duplicate vertices").clicked() {
				mesh.duplicate_vertices();
			}
		});
		ui.add_enabled_ui(mesh.indices().is_none(), |ui| {
			if ui.button("Compute flat normals").clicked() {
				mesh.compute_flat_normals();
			}
		});
		if ui.button("Generate tangents").clicked() {
			let _ = mesh.generate_tangents();
		}

		false
	}

	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, env: InspectorUi<'_, '_>) {
		let Some(Context { world, .. }) = env.context else {
			no_world_in_context(ui, "Handle<Mesh>");
			return;
		};

		let meshes = match world.get_resource_mut::<Assets<Mesh>>() {
			Ok(meshes) => meshes,
			Err(e) => {
				e.ui(ui, "Assets<Mesh>");
				return;
			}
		};
		let Some(mesh) = meshes.get(self) else {
			return dead_asset_handle(ui, self.id().untyped());
		};

		mesh_ui_inner(mesh, ui);
	}
}

impl InspectorPrimitive for Handle<Image> {
	fn ui(&mut self, ui: &mut egui::Ui, _: &dyn Any, id: egui::Id, env: InspectorUi<'_, '_>) -> bool {
		let Some(Context { world, .. }) = env.context else {
			let immutable_self: &Handle<Image> = self;
			no_world_in_context(ui, immutable_self.reflect_short_type_path());
			return false;
		};

		update_and_show_image(self, world, ui);
		let (asset_server, images) = match world.get_two_resources_mut::<AssetServer, Assets<Image>>() {
			(Ok(a), Ok(b)) => (a, b),
			(a, b) => {
				if let Err(e) = a {
					e.ui(ui, &pretty_type_name::<AssetServer>());
				}
				if let Err(e) = b {
					e.ui(ui, &pretty_type_name::<Assets<Image>>());
				}
				return false;
			}
		};

		// get all loaded image paths
		let mut image_paths = Vec::with_capacity(images.len());
		let mut handles = HashMap::new();
		for image in images.iter() {
			if let Some(image_path) = asset_server.get_path(image.0) {
				image_paths.push(image_path.to_string());
				handles.insert(image_path.to_string(), image.0);
			}
		}

		// first, get the typed search text from a stored egui data value
		let mut selected_path = None;
		let mut image_picker_search_text = String::from("");
		ui.data_mut(|data| {
			image_picker_search_text
				.clone_from(data.get_temp_mut_or_default::<String>(id.with("image_picker_search_text")));
		});

		// build and show the dropdown
		let dropdown = widgets::DropDownBox::from_iter(
			image_paths.iter(),
			id.with("image_picker"),
			&mut image_picker_search_text,
			|ui, path| {
				let response = ui
					.selectable_label(
						self
							.path()
							.is_some_and(|p| p.path().as_os_str().to_string_lossy().eq(path)),
						path,
					)
					.on_hover_ui_at_pointer(|ui| {
						if let Some(id) = handles.get(path) {
							let s: Option<SizedTexture> = ui.data(|d| d.get_temp(format!("image:{}", id).into()));
							if let Some(id) = s {
								ui.image(id);
							}
						}
					});
				if response.clicked() {
					selected_path = Some(path.to_string());
				}
				response
			},
		)
		.hint_text("Select image asset");
		ui.add_enabled(!image_paths.is_empty(), dropdown)
			.on_disabled_hover_text("No image assets are available");

		// update the typed search text
		ui.data_mut(|data| {
			*data.get_temp_mut_or_default::<String>(id.with("image_picker_search_text")) =
				image_picker_search_text;
		});

		// if the user selected an option, update the image handle
		if let Some(selected_path) = selected_path {
			*self = asset_server.load(selected_path);
		}

		false
	}

	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, env: InspectorUi<'_, '_>) {
		let Some(Context { world, .. }) = env.context else {
			no_world_in_context(ui, self.reflect_short_type_path());
			return;
		};

		update_and_show_image(self, world, ui);
	}
}

impl InspectorPrimitive for Color {
	fn ui(&mut self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) -> bool {
		match self {
			Color::Srgba(Srgba {
				red,
				green,
				blue,
				alpha,
			}) => {
				let mut color = egui::Color32::from_rgba_unmultiplied(
					(*red * 255.) as u8,
					(*green * 255.) as u8,
					(*blue * 255.) as u8,
					(*alpha * 255.) as u8,
				);
				if ui.color_edit_button_srgba(&mut color).changed() {
					let [r, g, b, a] = color.to_srgba_unmultiplied();
					*red = r as f32 / 255.;
					*green = g as f32 / 255.;
					*blue = b as f32 / 255.;
					*alpha = a as f32 / 255.;
					return true;
				}
			}
			Color::LinearRgba(LinearRgba {
				red,
				green,
				blue,
				alpha,
			}) => {
				let mut color = [*red, *green, *blue, *alpha];
				if ui
					.color_edit_button_rgba_premultiplied(&mut color)
					.changed()
				{
					*red = color[0];
					*green = color[1];
					*blue = color[2];
					*alpha = color[3];
					return true;
				}
			}
			Color::Hsla(Hsla {
				hue,
				saturation,
				lightness,
				alpha,
			}) => {
				let mut hsva = egui::ecolor::Hsva::new(*hue, *saturation, *lightness, *alpha);
				if ui.color_edit_button_hsva(&mut hsva).changed() {
					*hue = hsva.h;
					*saturation = hsva.s;
					*lightness = hsva.v;
					*alpha = hsva.a;
					return true;
				}
			}
			Color::Lcha(Lcha {
				hue,
				chroma,
				lightness,
				alpha,
			}) => {
				let mut hsva = egui::ecolor::Hsva::new(*hue, *chroma, *lightness, *alpha);
				if ui.color_edit_button_hsva(&mut hsva).changed() {
					*self = Color::Hsva(Hsva {
						hue: *hue,
						alpha: *alpha,
						saturation: *chroma,
						value: *lightness,
					});
					return true;
				}
			}
			Color::Hsva(_)
			| Color::Hwba(_)
			| Color::Laba(_)
			| Color::Oklaba(_)
			| Color::Oklcha(_)
			| Color::Xyza(_) => {
				ui.label(format!(
					"Colorspace of {self:?} is not supported yet. PRs welcome"
				));
				return false;
			}
		}
		false
	}

	fn ui_readonly(
		&self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, '_>,
	) {
		let mut copy = *self;
		ui.add_enabled_ui(false, |ui| copy.ui(ui, options, id, env));
	}
}

impl InspectorPrimitive for RenderLayers {
	fn ui(&mut self, ui: &mut egui::Ui, _: &dyn Any, id: egui::Id, _: InspectorUi<'_, '_>) -> bool {
		let mut new_value = None;
		egui::Grid::new(id).num_columns(2).show(ui, |ui| {
			for layer in self.iter() {
				let mut layer_copy = layer;
				if ui.add(egui::DragValue::new(&mut layer_copy)).changed() {
					new_value = Some(self.clone().without(layer).with(layer_copy));
				}

				if ui.button("-").clicked() {
					new_value = Some(self.clone().without(layer));
				}
				ui.end_row();
			}
		});

		ui.horizontal(|ui| {
			if ui.button("Add").clicked() {
				let new_layer = self.iter().last().map_or(0, |last| last + 1);
				new_value = Some(self.clone().with(new_layer));
			}
		});

		if let Some(new_value) = new_value {
			*self = new_value;
			true
		} else {
			false
		}
	}

	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) {
		for layer in self.iter() {
			ui.label(format!("- {layer}"));
		}
	}
}

impl InspectorPrimitive for Name {
	fn ui(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: InspectorUi<'_, '_>,
	) -> bool {
		let mut value = self.to_string();
		if value.ui(ui, options, id, env) {
			self.set(value);
			true
		} else {
			false
		}
	}

	fn ui_readonly(&self, ui: &mut egui::Ui, _: &dyn Any, _: egui::Id, _: InspectorUi<'_, '_>) {
		if self.contains('\n') {
			ui.text_edit_multiline(&mut self.as_str());
		} else {
			ui.text_edit_singleline(&mut self.as_str());
		}
	}
}

impl InspectorPrimitive for GizmoConfigStore {
	fn ui(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		mut env: InspectorUi<'_, '_>,
	) -> bool {
		for (ty, group, value) in self.iter_mut() {
			use egui::CollapsingHeader;

			let name = env
				.type_registry
				.get(*ty)
				.map(|x| x.type_info().ty().short_path())
				.unwrap_or("<unknown gizmo group>");
			CollapsingHeader::new(name)
				.id_salt(id.with(ty))
				.show(ui, |ui| {
					env.ui_for_reflect(group, ui);
					ui.separator();
					env.ui_for_reflect_with_options(value, ui, egui::Id::new("data"), &());
				});
		}

		false
	}
	fn ui_readonly(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		mut env: InspectorUi<'_, '_>,
	) {
		for (ty, group, value) in self.iter() {
			use egui::CollapsingHeader;

			let name = env
				.type_registry
				.get(*ty)
				.map(|x| x.type_info().ty().short_path())
				.unwrap_or("<unknown gizmo group>");
			CollapsingHeader::new(name)
				.id_salt(id.with(ty))
				.show(ui, |ui| {
					env.ui_for_reflect_readonly(group, ui);
					ui.separator();
					env.ui_for_reflect_readonly_with_options(value, ui, egui::Id::new("data"), &());
				});
		}
	}
}

fn update_and_show_image(
	image: &Handle<Image>,
	world: &mut RestrictedWorldView,
	ui: &mut egui::Ui,
) {
	let (mut egui_user_textures, mut images) =
		match world.get_two_resources_mut::<bevy_egui::EguiUserTextures, Assets<Image>>() {
			(Ok(a), Ok(b)) => (a, b),
			(a, b) => {
				if let Err(e) = a {
					e.ui(ui, &pretty_type_name::<bevy_egui::EguiContext>());
				}

				if let Err(e) = b {
					e.ui(ui, &pretty_type_name::<Assets<Image>>());
				}

				return;
			}
		};

	let mut scaled_down_textures = SCALED_DOWN_TEXTURES.lock();

	// todo: read asset events to re-rescale images if they changed
	let rescaled = rescaled_image(
		image,
		&mut scaled_down_textures,
		&mut images,
		&mut egui_user_textures,
	);
	let (rescaled_handle, texture_id) = match rescaled {
		Some(it) => it,
		None => {
			ui.label("<texture>");
			return;
		}
	};

	let rescaled_image = images.get(&rescaled_handle).unwrap();
	ui.data_mut(|d| {
		d.insert_temp(
			format!("image:{}", image.id()).into(),
			SizedTexture {
				id: texture_id,
				size: egui::Vec2::new(
					rescaled_image.texture_descriptor.size.width as f32,
					rescaled_image.texture_descriptor.size.height as f32,
				),
			},
		)
	});
	show_image(rescaled_image, texture_id, ui);
}

fn show_image(
	image: &Image,
	texture_id: egui::TextureId,
	ui: &mut egui::Ui,
) -> Option<egui::Response> {
	let size = image.texture_descriptor.size;
	let size = egui::Vec2::new(size.width as f32, size.height as f32);

	let source = SizedTexture {
		id: texture_id,
		size,
	};

	if size.max_elem() >= 128.0 {
		let response = egui::CollapsingHeader::new("Texture").show(ui, |ui| ui.image(source));
		response.body_response
	} else {
		let response = ui.image(source);
		Some(response)
	}
}

fn mesh_ui_inner(mesh: &Mesh, ui: &mut egui::Ui) {
	egui::Grid::new("mesh").show(ui, |ui| {
		ui.label("primitive_topology");
		ui.label(format!("{:?}", mesh.primitive_topology()));
		ui.end_row();

		ui.label("Vertices");
		ui.label(mesh.count_vertices().to_string());
		ui.end_row();

		if let Some(indices) = mesh.indices() {
			ui.label("Indices");
			let len = match indices {
				Indices::U16(vec) => vec.len(),
				Indices::U32(vec) => vec.len(),
			};
			ui.label(len.to_string());
			ui.end_row();
		}

		ui.label("Vertex Attributes");

		let builtin_attributes = &[
			Mesh::ATTRIBUTE_POSITION,
			Mesh::ATTRIBUTE_COLOR,
			Mesh::ATTRIBUTE_UV_0,
			Mesh::ATTRIBUTE_NORMAL,
			Mesh::ATTRIBUTE_TANGENT,
			Mesh::ATTRIBUTE_COLOR,
			Mesh::ATTRIBUTE_JOINT_INDEX,
			Mesh::ATTRIBUTE_JOINT_WEIGHT,
		];

		ui.vertical(|ui| {
			for attribute in builtin_attributes {
				if mesh.attribute(attribute.id).is_some() {
					ui.label(attribute.name);
				}
			}
		});
	});
}

#[derive(Default)]
struct ScaledDownTextures {
	textures: HashMap<Handle<Image>, Handle<Image>>,
	rescaled_textures: HashSet<Handle<Image>>,
}

const RESCALE_TO_FIT: (u32, u32) = (100, 100);

fn rescaled_image(
	handle: &Handle<Image>,
	scaled_down_textures: &mut ScaledDownTextures,
	textures: &mut Assets<Image>,
	egui_usere_textures: &mut EguiUserTextures,
) -> Option<(Handle<Image>, egui::TextureId)> {
	let (texture, texture_id) = match scaled_down_textures.textures.entry(handle.clone()) {
		hash_map::Entry::Occupied(handle) => {
			let handle: Handle<Image> = handle.get().clone();
			(
				handle.clone(),
				egui_usere_textures.add_image(EguiTextureHandle::Strong(handle)),
			)
		}
		hash_map::Entry::Vacant(entry) => {
			if scaled_down_textures.rescaled_textures.contains(handle) {
				return None;
			}

			let original = textures.get(handle)?;

			let (image, is_srgb) = image_texture_conversion::try_into_dynamic(original)?;
			let resized = image.resize(
				RESCALE_TO_FIT.0,
				RESCALE_TO_FIT.1,
				image::imageops::FilterType::Triangle,
			);
			let resized = image_texture_conversion::from_dynamic(resized, is_srgb);

			let resized_handle = textures.add(resized);
			let weak = resized_handle.clone();
			let texture_id =
				egui_usere_textures.add_image(EguiTextureHandle::Strong(resized_handle.clone()));
			entry.insert(resized_handle);
			scaled_down_textures.rescaled_textures.insert(weak.clone());

			(weak, texture_id)
		}
	};

	Some((texture, texture_id))
}
