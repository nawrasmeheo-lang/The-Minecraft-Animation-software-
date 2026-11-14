use eframe::egui;
use glam::{Mat4, Vec3};
use std::sync::{Arc, Mutex};

mod armature;
mod mesh;
mod render;

use armature::Armature;
use mesh::{EditableMesh, SelectMode};
use render::GlUnifiedRenderer;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractionMode {
    Object,
    Edit,
    Pose,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderEngineType {
    Workbench,
    EeveeCraft,
    CyclesCraft,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewportShading {
    Wireframe,
    Solid,
    MaterialPreview,
    Rendered,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropertiesTab {
    Render,
    World,
    Object,
    Modifiers,
    Armature,
    Material,
}

#[derive(Clone, Debug)]
pub struct AppPreferences {
    pub ui_scale: f32,
    pub left_click_select: bool,
    pub undo_steps: usize,
    pub memory_saver_4gb: bool,
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub show_grid: bool,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            left_click_select: true,
            undo_steps: 32,
            memory_saver_4gb: true,
            orbit_sensitivity: 0.01,
            zoom_sensitivity: 0.01,
            show_grid: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SceneObject {
    pub id: usize,
    pub name: String,
    pub mesh: Option<EditableMesh>,
    pub armature: Option<Armature>,
    pub position: Vec3,
    pub rotation_deg: Vec3,
    pub scale: Vec3,
    pub visible: bool,
}

impl SceneObject {
    pub fn world_matrix(&self) -> Mat4 {
        let r = self.rotation_deg * std::f32::consts::PI / 180.0;
        Mat4::from_scale_rotation_translation(
            self.scale,
            glam::Quat::from_euler(glam::EulerRot::XYZ, r.x, r.y, r.z),
            self.position,
        )
    }
}

#[derive(Clone)]
pub struct AppStateSnapshot {
    pub objects: Vec<SceneObject>,
    pub selected_id: Option<usize>,
    pub mode: InteractionMode,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("APPCRAFT 0.3 - Blender 3.6 Architecture"),
        renderer: eframe::Renderer::Glow,
        depth_buffer: 24,
        ..Default::default()
    };

    eframe::run_native(
        "APPCRAFT",
        options,
        Box::new(|cc| Ok(Box::new(AppCraftApp::new(cc)))),
    )
}

struct AppCraftApp {
    objects: Vec<SceneObject>,
    selected_id: Option<usize>,
    mode: InteractionMode,
    select_mode: SelectMode,
    engine: RenderEngineType,
    shading: ViewportShading,
    prop_tab: PropertiesTab,
    preferences: AppPreferences,
    show_preferences_window: bool,

    cam_target: Vec3,
    cam_dist: f32,
    cam_yaw: f32,
    cam_pitch: f32,

    undo_stack: Vec<AppStateSnapshot>,
    redo_stack: Vec<AppStateSnapshot>,

    modal_active: bool,
    modal_tool: u8,
    axis_lock: u8,
    modal_initial_pos: Vec3,
    modal_initial_rot: Vec3,
    modal_initial_scale: Vec3,

    renderer: Arc<Mutex<Option<GlUnifiedRenderer>>>,
    status_bar: String,
    next_id: usize,
}

impl AppCraftApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_blender_dark_theme(&cc.egui_ctx);
        let renderer = cc
            .gl
            .as_ref()
            .and_then(|gl| GlUnifiedRenderer::new(Arc::clone(gl)).ok());

        let mut app = Self {
            objects: Vec::new(),
            selected_id: None,
            mode: InteractionMode::Object,
            select_mode: SelectMode::Vertex,
            engine: RenderEngineType::Workbench,
            shading: ViewportShading::Solid,
            prop_tab: PropertiesTab::Render,
            preferences: AppPreferences::default(),
            show_preferences_window: false,

            cam_target: Vec3::new(0.0, 1.0, 0.0),
            cam_dist: 5.5,
            cam_yaw: 0.75,
            cam_pitch: 0.35,

            undo_stack: Vec::new(),
            redo_stack: Vec::new(),

            modal_active: false,
            modal_tool: 0,
            axis_lock: 0,
            modal_initial_pos: Vec3::ZERO,
            modal_initial_rot: Vec3::ZERO,
            modal_initial_scale: Vec3::ONE,

            renderer: Arc::new(Mutex::new(renderer)),
            status_bar: "Blender 3.6 Mode Active | Tab: Edit/Pose | G/R/S | Shift+A: Add".into(),
            next_id: 1,
        };

        app.spawn_steve_default();
        app
    }

    fn push_undo(&mut self) {
        if self.undo_stack.len() >= self.preferences.undo_steps {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(AppStateSnapshot {
            objects: self.objects.clone(),
            selected_id: self.selected_id,
            mode: self.mode,
        });
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(AppStateSnapshot {
                objects: self.objects.clone(),
                selected_id: self.selected_id,
                mode: self.mode,
            });
            self.objects = prev.objects;
            self.selected_id = prev.selected_id;
            self.mode = prev.mode;
            self.status_bar = "Undo".into();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(AppStateSnapshot {
                objects: self.objects.clone(),
                selected_id: self.selected_id,
                mode: self.mode,
            });
            self.objects = next.objects;
            self.selected_id = next.selected_id;
            self.mode = next.mode;
            self.status_bar = "Redo".into();
        }
    }

    fn spawn_steve_default(&mut self) {
        let id = self.next_id;
        self.next_id += 1;

        let mut cube = EditableMesh::new_cube(1.0);
        cube.subdivide();

        let mut arm = Armature::new_steve_armature();
        arm.calculate_auto_weights(&mut cube.vertices);

        self.objects.push(SceneObject {
            id,
            name: "Steve_Character".into(),
            mesh: Some(cube),
            armature: Some(arm),
            position: Vec3::ZERO,
            rotation_deg: Vec3::ZERO,
            scale: Vec3::ONE,
            visible: true,
        });
        self.selected_id = Some(id);
    }
}

impl eframe::App for AppCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_blender_dark_theme(ctx);

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Tab) {
                    self.push_undo();
                    self.mode = match self.mode {
                        InteractionMode::Object => {
                            if let Some(id) = self.selected_id {
                                if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                                    if obj.armature.is_some() && obj.mesh.is_none() {
                                        InteractionMode::Pose
                                    } else {
                                        InteractionMode::Edit
                                    }
                                } else { InteractionMode::Edit }
                            } else { InteractionMode::Edit }
                        }
                        InteractionMode::Edit => InteractionMode::Object,
                        InteractionMode::Pose => InteractionMode::Object,
                    };
                }

                if self.mode == InteractionMode::Edit {
                    if i.key_pressed(egui::Key::Num1) { self.select_mode = SelectMode::Vertex; }
                    if i.key_pressed(egui::Key::Num2) { self.select_mode = SelectMode::Edge; }
                    if i.key_pressed(egui::Key::Num3) { self.select_mode = SelectMode::Face; }

                    if i.key_pressed(egui::Key::E) {
                        self.push_undo();
                        if let Some(id) = self.selected_id {
                            if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                                if let Some(m) = &mut obj.mesh {
                                    if m.extrude_selected_faces() {
                                        self.status_bar = "Extruded Faces".into();
                                    }
                                }
                            }
                        }
                    }

                    if i.modifiers.ctrl && i.key_pressed(egui::Key::R) {
                        self.push_undo();
                        if let Some(id) = self.selected_id {
                            if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                                if let Some(m) = &mut obj.mesh {
                                    m.subdivide();
                                    self.status_bar = "Subdivided Mesh (Loop Prep)".into();
                                }
                            }
                        }
                    }
                }

                if i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z) {
                    self.undo();
                }
                if i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z) {
                    self.redo();
                }

                if i.modifiers.ctrl && i.modifiers.alt && i.key_pressed(egui::Key::U) {
                    self.show_preferences_window = true;
                }

                if i.key_pressed(egui::Key::A) {
                    let select = !i.modifiers.alt;
                    if let Some(id) = self.selected_id {
                        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                            if let Some(m) = &mut obj.mesh {
                                m.select_all(select);
                            }
                            if let Some(a) = &mut obj.armature {
                                for b in &mut a.bones { b.selected = select; }
                            }
                        }
                    }
                }

                if !self.modal_active {
                    if i.key_pressed(egui::Key::G) { self.start_modal(1); }
                    if i.key_pressed(egui::Key::R) { self.start_modal(2); }
                    if i.key_pressed(egui::Key::S) { self.start_modal(3); }
                } else {
                    if i.key_pressed(egui::Key::X) { self.axis_lock = 1; }
                    if i.key_pressed(egui::Key::Y) { self.axis_lock = 2; }
                    if i.key_pressed(egui::Key::Z) { self.axis_lock = 3; }
                    if i.key_pressed(egui::Key::Escape) { self.cancel_modal(); }
                    if i.key_pressed(egui::Key::Enter) { self.modal_active = false; }
                }
            });
        }

        egui::TopBottomPanel::top("blender_header").exact_height(28.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label("APPCRAFT");
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("New (General)").clicked() {
                        self.push_undo();
                        self.objects.clear();
                        self.spawn_steve_default();
                        ui.close_menu();
                    }
                    if ui.button("Preferences... (Ctrl+Alt+U)").clicked() {
                        self.show_preferences_window = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() { self.undo(); ui.close_menu(); }
                    if ui.button("Redo (Ctrl+Shift+Z)").clicked() { self.redo(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("Preferences...").clicked() {
                        self.show_preferences_window = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Add", |ui| {
                    if ui.button("Mesh -> Cube").clicked() {
                        self.push_undo();
                        let id = self.next_id; self.next_id += 1;
                        self.objects.push(SceneObject {
                            id,
                            name: format!("Cube.{:03}", id),
                            mesh: Some(EditableMesh::new_cube(1.0)),
                            armature: None,
                            position: Vec3::ZERO,
                            rotation_deg: Vec3::ZERO,
                            scale: Vec3::ONE,
                            visible: true,
                        });
                        self.selected_id = Some(id);
                        ui.close_menu();
                    }
                    if ui.button("Armature -> Steve Rig").clicked() {
                        self.push_undo();
                        let id = self.next_id; self.next_id += 1;
                        self.objects.push(SceneObject {
                            id,
                            name: "Steve_Armature".into(),
                            mesh: None,
                            armature: Some(Armature::new_steve_armature()),
                            position: Vec3::ZERO,
                            rotation_deg: Vec3::ZERO,
                            scale: Vec3::ONE,
                            visible: true,
                        });
                        self.selected_id = Some(id);
                        ui.close_menu();
                    }
                });

                ui.separator();
                let mode_str = match self.mode {
                    InteractionMode::Object => "Object Mode",
                    InteractionMode::Edit => "Edit Mode",
                    InteractionMode::Pose => "Pose Mode",
                };
                egui::ComboBox::from_id_salt("mode_sel").selected_text(mode_str).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.mode, InteractionMode::Object, "Object Mode");
                    ui.selectable_value(&mut self.mode, InteractionMode::Edit, "Edit Mode");
                    ui.selectable_value(&mut self.mode, InteractionMode::Pose, "Pose Mode");
                });

                if self.mode == InteractionMode::Edit {
                    ui.separator();
                    ui.selectable_value(&mut self.select_mode, SelectMode::Vertex, "1: Vert");
                    ui.selectable_value(&mut self.select_mode, SelectMode::Edge, "2: Edge");
                    ui.selectable_value(&mut self.select_mode, SelectMode::Face, "3: Face");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut self.shading, ViewportShading::Rendered, "Render");
                    ui.selectable_value(&mut self.shading, ViewportShading::MaterialPreview, "Material");
                    ui.selectable_value(&mut self.shading, ViewportShading::Solid, "Solid");
                    ui.selectable_value(&mut self.shading, ViewportShading::Wireframe, "Wire");
                });
            });
        });

        egui::TopBottomPanel::bottom("status_bar").exact_height(20.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(&self.status_bar);
            });
        });

        egui::SidePanel::right("blender_properties").default_width(280.0).resizable(true).show(ctx, |ui| {
            ui.heading("Outliner");
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                for obj in &mut self.objects {
                    let sel = self.selected_id == Some(obj.id);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut obj.visible, "");
                        if ui.selectable_label(sel, &obj.name).clicked() {
                            self.selected_id = Some(obj.id);
                        }
                    });
                }
            });

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (PropertiesTab::Render, "Render"),
                    (PropertiesTab::World, "World"),
                    (PropertiesTab::Object, "Object"),
                    (PropertiesTab::Modifiers, "Mod"),
                    (PropertiesTab::Armature, "Bone"),
                    (PropertiesTab::Material, "Mat"),
                ] {
                    if ui.selectable_label(self.prop_tab == tab, label).clicked() {
                        self.prop_tab = tab;
                    }
                }
            });
            ui.separator();

            match self.prop_tab {
                PropertiesTab::Render => {
                    ui.label("Render Engine:");
                    ui.selectable_value(&mut self.engine, RenderEngineType::Workbench, "Workbench (Fast 60fps)");
                    ui.selectable_value(&mut self.engine, RenderEngineType::EeveeCraft, "Eevee Craft (PBR / Shadows)");
                    ui.selectable_value(&mut self.engine, RenderEngineType::CyclesCraft, "Cycles Craft (CPU GI / Tiles)");
                }
                PropertiesTab::Object => {
                    if let Some(id) = self.selected_id {
                        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                            ui.label(&obj.name);
                            ui.horizontal(|ui| {
                                ui.label("Pos:");
                                ui.add(egui::DragValue::new(&mut obj.position.x).speed(0.05).prefix("X: "));
                                ui.add(egui::DragValue::new(&mut obj.position.y).speed(0.05).prefix("Y: "));
                                ui.add(egui::DragValue::new(&mut obj.position.z).speed(0.05).prefix("Z: "));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Rot:");
                                ui.add(egui::DragValue::new(&mut obj.rotation_deg.x).speed(1.0).prefix("X: "));
                                ui.add(egui::DragValue::new(&mut obj.rotation_deg.y).speed(1.0).prefix("Y: "));
                                ui.add(egui::DragValue::new(&mut obj.rotation_deg.z).speed(1.0).prefix("Z: "));
                            });
                        }
                    }
                }
                PropertiesTab::Armature => {
                    if let Some(id) = self.selected_id {
                        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                            if let Some(arm) = &mut obj.armature {
                                ui.label(format!("Bones: {}", arm.bones.len()));
                                for ik in &mut arm.ik_constraints {
                                    if ui.checkbox(&mut ik.enabled, "Enable Leg 2-Bone IK").changed() {
                                        arm.solve_ik();
                                    }
                                }
                                if ui.button("Clear Pose Transforms (Alt+R)").clicked() {
                                    arm.clear_pose_transforms();
                                }
                            }
                        }
                    }
                }
                _ => {
                    ui.label("Blender Properties Active");
                }
            }
        });

        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

            if !self.modal_active {
                if response.dragged_by(egui::PointerButton::Middle) {
                    let d = response.drag_delta();
                    if ctx.input(|i| i.modifiers.shift) {
                        self.cam_target.x -= d.x * 0.01;
                        self.cam_target.y += d.y * 0.01;
                    } else {
                        self.cam_yaw -= d.x * self.preferences.orbit_sensitivity;
                        self.cam_pitch = (self.cam_pitch + d.y * self.preferences.orbit_sensitivity).clamp(-1.45, 1.45);
                    }
                }
                if response.hovered() {
                    let s = ctx.input(|i| i.raw_scroll_delta.y);
                    if s.abs() > 0.0 {
                        self.cam_dist = (self.cam_dist - s * self.preferences.zoom_sensitivity).clamp(1.0, 100.0);
                    }
                }
            } else {
                let d = ctx.input(|i| i.pointer.delta());
                self.apply_modal_delta(d);
                if response.clicked() { self.modal_active = false; }
            }

            let ppp = ctx.pixels_per_point();
            let screen_h = (ctx.screen_rect().height() * ppp) as i32;
            let vp_rect = [
                (rect.min.x * ppp) as i32,
                screen_h - (rect.max.y * ppp) as i32,
                (rect.width() * ppp).max(1.0) as i32,
                (rect.height() * ppp).max(1.0) as i32,
            ];

            let aspect = rect.width() / rect.height().max(1.0);
            let eye = self.cam_target + Vec3::new(
                self.cam_dist * self.cam_pitch.cos() * self.cam_yaw.sin(),
                self.cam_dist * self.cam_pitch.sin(),
                self.cam_dist * self.cam_pitch.cos() * self.cam_yaw.cos(),
            );
            let view = Mat4::look_at_rh(eye, self.cam_target, Vec3::Y);
            let proj = Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 500.0);
            let vp_matrix = proj * view;

            let renderer_arc = Arc::clone(&self.renderer);
            let objects_clone = self.objects.clone();
            let sel_id = self.selected_id;
            let mode = self.mode;
            let select_mode = self.select_mode;
            let show_grid = self.preferences.show_grid;

            let cb = egui::PaintCallback {
                rect,
                callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |_info, _painter| {
                    if let Ok(guard) = renderer_arc.lock() {
                        if let Some(r) = guard.as_ref() {
                            r.begin_frame(vp_rect);
                            if show_grid { r.draw_grid(&vp_matrix); }

                            for obj in &objects_clone {
                                if !obj.visible { continue; }
                                let world = obj.world_matrix();
                                let is_selected = sel_id == Some(obj.id);

                                if let Some(m) = &obj.mesh {
                                    r.draw_editable_mesh(
                                        m,
                                        &world,
                                        &vp_matrix,
                                        is_selected,
                                        mode == InteractionMode::Edit && is_selected,
                                        select_mode,
                                    );
                                }

                                if let Some(arm) = &obj.armature {
                                    r.draw_armature_xray(
                                        arm,
                                        &world,
                                        &vp_matrix,
                                        mode == InteractionMode::Pose && is_selected,
                                    );
                                }
                            }

                            r.end_frame();
                        }
                    }
                })),
            };
            ui.painter().add(cb);
        });

        if self.show_preferences_window {
            egui::Window::new("Blender Preferences")
                .open(&mut self.show_preferences_window)
                .default_size([550.0, 380.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Memory (4GB RAM Mode):");
                        ui.checkbox(&mut self.preferences.memory_saver_4gb, "Optimize VRAM / LZ4 Cache");
                    });
                    ui.add(egui::Slider::new(&mut self.preferences.undo_steps, 8..=64).text("Undo Steps"));
                    ui.add(egui::Slider::new(&mut self.preferences.orbit_sensitivity, 0.002..=0.03).text("Orbit Sensitivity"));
                    ui.checkbox(&mut self.preferences.left_click_select, "Left Click Select (Blender 2.8+)");
                    ui.checkbox(&mut self.preferences.show_grid, "Show Viewport Grid");
                });
        }
    }
}

impl AppCraftApp {
    fn start_modal(&mut self, tool: u8) {
        if let Some(id) = self.selected_id {
            if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                self.push_undo();
                self.modal_active = true;
                self.modal_tool = tool;
                self.axis_lock = 0;
                self.modal_initial_pos = obj.position;
                self.modal_initial_rot = obj.rotation_deg;
                self.modal_initial_scale = obj.scale;
            }
        }
    }

    fn cancel_modal(&mut self) {
        if let Some(id) = self.selected_id {
            if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                obj.position = self.modal_initial_pos;
                obj.rotation_deg = self.modal_initial_rot;
                obj.scale = self.modal_initial_scale;
            }
        }
        self.modal_active = false;
        self.axis_lock = 0;
    }

    fn apply_modal_delta(&mut self, delta: egui::Vec2) {
        if let Some(id) = self.selected_id {
            if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                let s = 0.02;
                match self.modal_tool {
                    1 => {
                        match self.axis_lock {
                            1 => obj.position.x += delta.x * s,
                            2 => obj.position.y -= delta.y * s,
                            3 => obj.position.z += delta.y * s,
                            _ => { obj.position.x += delta.x * s; obj.position.y -= delta.y * s; }
                        }
                    }
                    2 => {
                        let rs = 0.8;
                        match self.axis_lock {
                            1 => obj.rotation_deg.x += delta.x * rs,
                            2 => obj.rotation_deg.y += delta.x * rs,
                            3 => obj.rotation_deg.z += delta.x * rs,
                            _ => obj.rotation_deg.y += delta.x * rs,
                        }
                    }
                    3 => {
                        let sc = delta.x * 0.01;
                        match self.axis_lock {
                            1 => obj.scale.x = (obj.scale.x + sc).max(0.01),
                            2 => obj.scale.y = (obj.scale.y + sc).max(0.01),
                            3 => obj.scale.z = (obj.scale.z + sc).max(0.01),
                            _ => {
                                for i in 0..3 {
                                    let v = match i { 0 => &mut obj.scale.x, 1 => &mut obj.scale.y, _ => &mut obj.scale.z };
                                    *v = (*v + sc).max(0.01);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn apply_blender_dark_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_rgb(46, 46, 46);
    style.visuals.window_fill = egui::Color32::from_rgb(46, 46, 46);
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(58, 58, 58);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(70, 70, 70);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(90, 90, 90);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(71, 114, 179);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(71, 114, 179);
    ctx.set_style(style);
                                          }
