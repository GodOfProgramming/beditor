use super::InspectorPrimitive;
use crate::{
	inspector::{
		errors::dead_asset_handle,
		options::{EntityDisplay, EntityOptions},
		ui::{ImmutableContext, InspectorUi, MutableContext},
	},
	ui::builtin::inspector::entity_context_menu,
	util::{
		self, pretty_type_name,
		world::{MutableWorldView, RestrictedWorldView},
	},
};
use bevy::{
	camera::visibility::RenderLayers, ecs::world::CommandQueue, gizmos::config::GizmoConfigStore,
	mesh::Indices, platform::collections::HashMap, prelude::*,
};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use egui::{Widget, load::SizedTexture};
use std::any::Any;

impl InspectorPrimitive for uuid::Uuid {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		ui.label(self.to_string());
		false
	}

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		ui.label(self.to_string());
	}
}

impl InspectorPrimitive for Entity {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
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
				let ctx = &mut env.context;

				let entity_name = util::entity::guess_entity_name_restricted(&ctx.world_view, entity);

				egui::CollapsingHeader::new(entity_name)
					.id_salt(id)
					.show(ui, |ui| {
						let maybe_response = crate::inspector::ui::components::ui_for_entity_components(
							ctx,
							entity,
							ui,
							id,
							env.type_registry,
							options.highlight_changes,
						);

						if let Some(response) = maybe_response {
							entity_context_menu(&response, ctx.queue, std::iter::once(entity));
						}

						if options.despawnable
							&& ctx.world_view.contains_entity(entity)
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

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		ui.label(format!("{self:?}"));
	}
}

impl InspectorPrimitive for Handle<Mesh> {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		asset_picker(
			self,
			ui,
			env,
			id,
			|_, _, _, _| {},
			|_, _, _| {},
			|ui, handle, meshes| {
				ui.horizontal(|ui| {
					let Some(mesh) = meshes.get_mut(handle.id()) else {
						dead_asset_handle(ui, handle.id().untyped());
						return false;
					};

					mesh_ui_inner(mesh, ui);

					let mut changed = false;

					ui.add_enabled_ui(mesh.indices().is_some(), |ui| {
						if ui.button("Duplicate vertices").clicked() {
							mesh.duplicate_vertices();
							changed |= true;
						}
					});

					ui.add_enabled_ui(mesh.indices().is_none(), |ui| {
						if ui.button("Compute flat normals").clicked() {
							mesh.compute_flat_normals();
							changed |= true;
						}
					});

					if ui.button("Generate tangents").clicked() {
						let _ = mesh.generate_tangents();
						changed |= true;
					}

					changed
				})
				.inner
			},
		)
	}

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let ImmutableContext {
			world_view: world, ..
		} = env.context;

		let meshes = match world.resource::<Assets<Mesh>>() {
			Ok(meshes) => meshes,
			Err(err) => {
				err.ui(ui, "Assets<Mesh>");
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
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		asset_picker(
			self,
			ui,
			env,
			id,
			|ui, handle, world, queue| {
				let egui_user_textures = match world.resource::<EguiUserTextures>() {
					Ok(v) => v,
					Err(err) => {
						err.ui(ui, "EguiUserTextures");
						return;
					}
				};

				show_image(ui, handle, queue, egui_user_textures);
			},
			|ui, search_text, handles| {
				if let Some(id) = handles.get(search_text) {
					let tex: Option<SizedTexture> = ui.data(|d| d.get_temp(format!("image:{}", id).into()));
					if let Some(tex) = tex {
						ui.image(tex);
					}
				}
			},
			|_, _, _| false,
		)
	}

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		let ImmutableContext {
			world_view: world,
			queue,
		} = env.context;

		let mut queue = queue.borrow_mut();
		let egui_user_textures = match world.resource::<EguiUserTextures>() {
			Ok(res) => res,
			Err(err) => {
				err.ui(ui, "EguiUserTextures");
				return;
			}
		};

		show_image(ui, self, &mut queue, egui_user_textures);
	}
}

impl InspectorPrimitive for Color {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
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

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		ui.add_enabled_ui(false, |ui| match self {
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
				ui.color_edit_button_srgba(&mut color);
			}
			Color::LinearRgba(LinearRgba {
				red,
				green,
				blue,
				alpha,
			}) => {
				let mut color = [*red, *green, *blue, *alpha];
				ui.color_edit_button_rgba_premultiplied(&mut color);
			}
			Color::Hsla(Hsla {
				hue,
				saturation,
				lightness,
				alpha,
			}) => {
				let mut hsva = egui::ecolor::Hsva::new(*hue, *saturation, *lightness, *alpha);
				ui.color_edit_button_hsva(&mut hsva);
			}
			Color::Lcha(Lcha {
				hue,
				chroma,
				lightness,
				alpha,
			}) => {
				let mut hsva = egui::ecolor::Hsva::new(*hue, *chroma, *lightness, *alpha);
				ui.color_edit_button_hsva(&mut hsva);
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
			}
		});
	}
}

impl InspectorPrimitive for RenderLayers {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		_: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
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

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		for layer in self.iter() {
			ui.label(format!("- {layer}"));
		}
	}
}

impl InspectorPrimitive for Name {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		options: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
	) -> bool {
		let mut value = self.to_string();
		if value.ui(ui, options, id, env) {
			self.set(value);
			true
		} else {
			false
		}
	}

	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		_: egui::Id,
		_: &InspectorUi<'_, ImmutableContext<'c>>,
	) {
		if self.contains('\n') {
			ui.text_edit_multiline(&mut self.as_str());
		} else {
			ui.text_edit_singleline(&mut self.as_str());
		}
	}
}

impl InspectorPrimitive for GizmoConfigStore {
	fn ui<'c>(
		&mut self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &mut InspectorUi<'_, MutableContext<'c>>,
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
	fn ui_readonly<'c>(
		&self,
		ui: &mut egui::Ui,
		_: &dyn Any,
		id: egui::Id,
		env: &InspectorUi<'_, ImmutableContext<'c>>,
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

fn show_image(
	ui: &mut egui::Ui,
	handle: &Handle<Image>,
	queue: &mut CommandQueue,
	egui_user_textures: &EguiUserTextures,
) -> Option<egui::Response> {
	let Some(tex) = egui_user_textures.image_id(handle.id()) else {
		queue.push(MakeEguiTexture(handle.clone()));
		return None;
	};

	let source = SizedTexture {
		id: tex,
		size: egui::Vec2::new(64.0, 64.0),
	};

	Some(ui.image(source))
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

#[derive(Deref)]
struct MakeEguiTexture(Handle<Image>);

impl Command for MakeEguiTexture {
	fn apply(self, world: &mut World) {
		let mut egui_user_textures = world.resource_mut::<EguiUserTextures>();
		egui_user_textures.add_image(EguiTextureHandle::Weak(self.id()));
	}
}

fn asset_picker<'c, A: Asset>(
	handle: &mut Handle<A>,
	ui: &mut egui::Ui,
	env: &mut InspectorUi<'_, MutableContext<'c>>,
	id: egui::Id,
	prefix_ui: impl FnOnce(
		&mut egui::Ui,
		&mut Handle<A>,
		&mut RestrictedWorldView<MutableWorldView<'c>>,
		&mut CommandQueue,
	),
	hover_ui: impl FnOnce(&mut egui::Ui, &str, &HashMap<String, AssetId<A>>),
	postfix_ui: impl FnOnce(&mut egui::Ui, &mut Handle<A>, &mut Assets<A>) -> bool,
) -> bool {
	let MutableContext { world_view, queue } = env.context;

	(prefix_ui)(ui, handle, world_view, queue);

	let (asset_server, mut assets) = match world_view.two_resources_mut::<AssetServer, Assets<A>>() {
		(Ok(a), Ok(b)) => (a, b),
		(a, b) => {
			if let Err(e) = a {
				e.ui(ui, &pretty_type_name::<AssetServer>());
			}

			if let Err(e) = b {
				e.ui(ui, &pretty_type_name::<Assets<A>>());
			}
			return false;
		}
	};

	let mut paths = Vec::with_capacity(assets.len());
	let mut handles = HashMap::with_capacity(assets.len());
	for asset_id in assets.iter().map(|a| a.0) {
		if let Some(mesh_path) = asset_server.get_path(asset_id) {
			paths.push(mesh_path.to_string());
			handles.insert(mesh_path.to_string(), asset_id);
		}
	}

	let search_id = id.with("search_text");
	let mut search_text = String::new();

	ui.data_mut(|data| {
		search_text.clone_from(data.get_temp_mut_or_default::<String>(search_id));
	});

	ui.vertical(|ui| {
		ui.add_enabled_ui(!paths.is_empty(), |ui| {
			let response = egui_autocomplete::AutoCompleteTextEdit::new(&mut search_text, paths)
				.popup_on_focus(true)
				.ui(ui)
				.on_hover_ui_at_pointer(|ui| (hover_ui)(ui, &search_text, &handles));

			if response.lost_focus()
				&& let Some(new_handle) = handles
					.get(&search_text)
					.and_then(|&id| assets.get_strong_handle(id))
			{
				*handle = new_handle;
			}
		})
		.response
		.on_disabled_hover_text("No assets are available");

		// update the typed search text
		ui.data_mut(|data| {
			*data.get_temp_mut_or_default::<String>(search_id) = search_text;
		});

		(postfix_ui)(ui, handle, &mut assets)
	})
	.inner
}
