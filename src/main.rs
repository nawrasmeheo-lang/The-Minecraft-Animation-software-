use eframe::egui;
use glam::{Mat4, Vec3};
use std::sync::{Arc, Mutex};

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

#[derive(Clone, Debug)]
pub struct SceneObject {
    pub id: usize,
    pub name: String,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
    pub color: [f32; 3],
    pub visible: bool,
}

impl SceneObject {
    pub fn matrix(&self) -> Mat4 {
        let r = self.rotation * std::f32::consts::PI / 180.0;
        Mat4::from_scale_rotation_translation(
            self.scale,
            glam::Quat::from_euler(glam::EulerRot::XYZ, r.x, r.y, r.z),
            self.position,
        )
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("APPCRAFT - Blender Architecture"),
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
    engine: RenderEngineType,
    cam_target: Vec3,
    cam_dist: f32,
    cam_yaw: f32,
    cam_pitch: f32,
    current_frame: i32,
    playing: bool,
    show_preferences: bool,
    gl_data: Arc<Mutex<Option<GlRenderer>>>,
    status: String,
    next_id: usize,
}

impl AppCraftApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_blender_theme(&cc.egui_ctx);
        let gl_data = cc.gl.as_ref().and_then(|gl| GlRenderer::new(Arc::clone(gl)).ok());

        let mut app = Self {
            objects: Vec::new(),
            selected_id: None,
            mode: InteractionMode::Object,
            engine: RenderEngineType::Workbench,
            cam_target: Vec3::new(0.0, 1.0, 0.0),
            cam_dist: 6.0,
            cam_yaw: 0.75,
            cam_pitch: 0.35,
            current_frame: 1,
            playing: false,
            show_preferences: false,
            gl_data: Arc::new(Mutex::new(gl_data)),
            status: "Ready | Tab: Mode | G/R/S | Space: Play".into(),
            next_id: 1,
        };

        app.spawn_steve();
        app
    }

    fn spawn_steve(&mut self) {
        let parts = [
            ("Steve_Head", Vec3::new(0.0, 1.75, 0.0), Vec3::new(0.5, 0.5, 0.5), [0.85, 0.65, 0.52]),
            ("Steve_Body", Vec3::new(0.0, 1.125, 0.0), Vec3::new(0.5, 0.75, 0.25), [0.18, 0.62, 0.72]),
            ("Steve_Arm_R", Vec3::new(0.375, 1.125, 0.0), Vec3::new(0.25, 0.75, 0.25), [0.85, 0.65, 0.52]),
            ("Steve_Arm_L", Vec3::new(-0.375, 1.125, 0.0), Vec3::new(0.25, 0.75, 0.25), [0.85, 0.65, 0.52]),
            ("Steve_Leg_R", Vec3::new(0.125, 0.375, 0.0), Vec3::new(0.25, 0.75, 0.25), [0.22, 0.25, 0.65]),
            ("Steve_Leg_L", Vec3::new(-0.125, 0.375, 0.0), Vec3::new(0.25, 0.75, 0.25), [0.22, 0.25, 0.65]),
        ];

        for (name, pos, sc, col) in parts {
            let id = self.next_id;
            self.next_id += 1;
            self.objects.push(SceneObject {
                id,
                name: name.into(),
                position: pos,
                rotation: Vec3::ZERO,
                scale: sc,
                color: col,
                visible: true,
            });
        }
        self.selected_id = Some(1);
    }
}

impl eframe::App for AppCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_blender_theme(ctx);

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Tab) {
                    self.mode = match self.mode {
                        InteractionMode::Object => InteractionMode::Edit,
                        InteractionMode::Edit => InteractionMode::Pose,
                        InteractionMode::Pose => InteractionMode::Object,
                    };
                }
                if i.key_pressed(egui::Key::Space) {
                    self.playing = !self.playing;
                }
            });
        }

        if self.playing {
            self.current_frame += 1;
            if self.current_frame > 250 { self.current_frame = 1; }
            ctx.request_repaint();
        }

        // Top Header
        egui::TopBottomPanel::top("top").exact_height(28.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label("APPCRAFT");
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.objects.clear();
                        self.spawn_steve();
                        ui.close_menu();
                    }
                    if ui.button("Preferences...").clicked() {
                        self.show_preferences = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Add", |ui| {
                    if ui.button("Cube").clicked() {
                        let id = self.next_id; self.next_id += 1;
                        self.objects.push(SceneObject {
                            id,
                            name: format!("Cube.{:03}", id),
                            position: Vec3::ZERO,
                            rotation: Vec3::ZERO,
                            scale: Vec3::ONE,
                            color: [0.7, 0.7, 0.7],
                            visible: true,
                        });
                        self.selected_id = Some(id);
                        ui.close_menu();
                    }
                });

                ui.separator();
                egui::ComboBox::from_id_salt("mode_combo")
                    .selected_text(format!("{:?}", self.mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.mode, InteractionMode::Object, "Object Mode");
                        ui.selectable_value(&mut self.mode, InteractionMode::Edit, "Edit Mode");
                        ui.selectable_value(&mut self.mode, InteractionMode::Pose, "Pose Mode");
                    });

                ui.separator();
                ui.label("Engine:");
                ui.selectable_value(&mut self.engine, RenderEngineType::Workbench, "Workbench");
                ui.selectable_value(&mut self.engine, RenderEngineType::EeveeCraft, "Eevee");
                ui.selectable_value(&mut self.engine, RenderEngineType::CyclesCraft, "Cycles");
            });
        });

        // Bottom Timeline
        egui::TopBottomPanel::bottom("timeline").exact_height(70.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.playing { "Pause" } else { "Play" }).clicked() {
                    self.playing = !self.playing;
                }
                ui.label(format!("Frame: {}", self.current_frame));
            });
            ui.add(egui::Slider::new(&mut self.current_frame, 1..=250).show_value(false));
        });

        // Right Panel (Outliner + Properties)
        egui::SidePanel::right("props").default_width(260.0).show(ctx, |ui| {
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
            ui.heading("Transform");
            if let Some(id) = self.selected_id {
                if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
                    ui.label(&obj.name);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut obj.position.x).speed(0.05).prefix("X: "));
                        ui.add(egui::DragValue::new(&mut obj.position.y).speed(0.05).prefix("Y: "));
                        ui.add(egui::DragValue::new(&mut obj.position.z).speed(0.05).prefix("Z: "));
                    });
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut obj.rotation.x).speed(1.0).prefix("Rot X: "));
                        ui.add(egui::DragValue::new(&mut obj.rotation.y).speed(1.0).prefix("Rot Y: "));
                        ui.add(egui::DragValue::new(&mut obj.rotation.z).speed(1.0).prefix("Rot Z: "));
                    });
                    ui.color_edit_button_rgb(&mut obj.color);
                }
            }
        });

        // Viewport
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, egui::Sense::drag());

            if response.dragged_by(egui::PointerButton::Middle) {
                let d = response.drag_delta();
                self.cam_yaw -= d.x * 0.01;
                self.cam_pitch = (self.cam_pitch + d.y * 0.01).clamp(-1.45, 1.45);
            }
            if response.hovered() {
                let s = ctx.input(|i| i.raw_scroll_delta.y);
                if s.abs() > 0.0 {
                    self.cam_dist = (self.cam_dist - s * 0.01).clamp(1.5, 50.0);
                }
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
            let vp = proj * view;

            let gl_data = Arc::clone(&self.gl_data);
            let objects_clone = self.objects.clone();
            let sel_id = self.selected_id;

            let cb = egui::PaintCallback {
                rect,
                callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |_info, _painter| {
                    if let Ok(guard) = gl_data.lock() {
                        if let Some(r) = guard.as_ref() {
                            r.render(vp_rect, &vp, &objects_clone, sel_id);
                        }
                    }
                })),
            };
            ui.painter().add(cb);
        });

        // Preferences Window
        if self.show_preferences {
            egui::Window::new("Preferences").open(&mut self.show_preferences).show(ctx, |ui| {
                ui.label("Memory Saver (4GB RAM): Active");
                ui.label("Target Device: Core i3 IvyBridge + HD 4000");
                ui.label("OpenGL Version: 3.3 Core");
            });
        }
    }
}

// ---------------- Minimal OpenGL 3.3 Renderer ----------------
struct GlRenderer {
    gl: Arc<egui_glow::glow::Context>,
    prog: egui_glow::glow::Program,
    cube_vao: egui_glow::glow::VertexArray,
    cube_vbo: egui_glow::glow::Buffer,
    grid_vao: egui_glow::glow::VertexArray,
    grid_vbo: egui_glow::glow::Buffer,
    grid_count: i32,
}

impl GlRenderer {
    fn new(gl: Arc<egui_glow::glow::Context>) -> Result<Self, String> {
        use egui_glow::glow::HasContext;
        unsafe {
            let vs_src = "#version 330 core\nlayout(location=0) in vec3 pos; layout(location=1) in vec3 norm; uniform mat4 mvp; uniform mat4 model; out vec3 n; void main(){ gl_Position=mvp*vec4(pos,1.0); n=mat3(model)*norm; }";
            let fs_src = "#version 330 core\nin vec3 n; uniform vec3 col; uniform float sel; out vec4 fc; void main(){ vec3 sun=normalize(vec3(0.5,0.8,0.4)); float d=max(dot(normalize(n),sun),0.0); vec3 c=col*(0.4+0.6*d); if(sel>0.5) c=mix(c,vec3(1.0,0.5,0.0),0.3); fc=vec4(c,1.0); }";

            let prog = compile_shader(&gl, vs_src, fs_src)?;
            let (cube_vao, cube_vbo) = create_cube(&gl)?;
            let (grid_vao, grid_vbo, grid_count) = create_grid(&gl, 12)?;

            Ok(Self { gl, prog, cube_vao, cube_vbo, grid_vao, grid_vbo, grid_count })
        }
    }

    fn render(&self, vp_rect: [i32; 4], vp: &Mat4, objs: &[SceneObject], sel_id: Option<usize>) {
        use egui_glow::glow::HasContext;
        let [x, y, w, h] = vp_rect;
        if w <= 0 || h <= 0 { return; }

        unsafe {
            let gl = &self.gl;
            gl.viewport(x, y, w, h);
            gl.enable(egui_glow::glow::DEPTH_TEST);
            gl.clear(egui_glow::glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.prog));
            let mvp_loc = gl.get_uniform_location(self.prog, "mvp");
            let model_loc = gl.get_uniform_location(self.prog, "model");
            let col_loc = gl.get_uniform_location(self.prog, "col");
            let sel_loc = gl.get_uniform_location(self.prog, "sel");

            // Draw Grid
            gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &vp.to_cols_array());
            gl.uniform_matrix_4_f32_slice(model_loc.as_ref(), false, &Mat4::IDENTITY.to_cols_array());
            gl.uniform_3_f32(col_loc.as_ref(), 0.35, 0.35, 0.35);
            gl.uniform_1_f32(sel_loc.as_ref(), 0.0);
            gl.bind_vertex_array(Some(self.grid_vao));
            gl.draw_arrays(egui_glow::glow::LINES, 0, self.grid_count);

            // Draw Objects
            gl.bind_vertex_array(Some(self.cube_vao));
            for obj in objs {
                if !obj.visible { continue; }
                let model = obj.matrix();
                let mvp = *vp * model;
                gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mvp.to_cols_array());
                gl.uniform_matrix_4_f32_slice(model_loc.as_ref(), false, &model.to_cols_array());
                gl.uniform_3_f32(col_loc.as_ref(), obj.color[0], obj.color[1], obj.color[2]);
                let sel = if sel_id == Some(obj.id) { 1.0 } else { 0.0 };
                gl.uniform_1_f32(sel_loc.as_ref(), sel);
                gl.draw_arrays(egui_glow::glow::TRIANGLES, 0, 36);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
            gl.disable(egui_glow::glow::DEPTH_TEST);
        }
    }
}

unsafe fn compile_shader(gl: &egui_glow::glow::Context, vs_src: &str, fs_src: &str) -> Result<egui_glow::glow::Program, String> {
    use egui_glow::glow::HasContext;
    let p = gl.create_program()?;
    let vs = gl.create_shader(egui_glow::glow::VERTEX_SHADER)?;
    gl.shader_source(vs, vs_src);
    gl.compile_shader(vs);
    let fs = gl.create_shader(egui_glow::glow::FRAGMENT_SHADER)?;
    gl.shader_source(fs, fs_src);
    gl.compile_shader(fs);
    gl.attach_shader(p, vs);
    gl.attach_shader(p, fs);
    gl.link_program(p);
    Ok(p)
}

unsafe fn create_cube(gl: &egui_glow::glow::Context) -> Result<(egui_glow::glow::VertexArray, egui_glow::glow::Buffer), String> {
    use egui_glow::glow::HasContext;
    #[rustfmt::skip]
    let v: [f32; 216] = [
        -0.5,-0.5, 0.5, 0.0,0.0,1.0,  0.5,-0.5, 0.5, 0.0,0.0,1.0,  0.5, 0.5, 0.5, 0.0,0.0,1.0,
        -0.5,-0.5, 0.5, 0.0,0.0,1.0,  0.5, 0.5, 0.5, 0.0,0.0,1.0, -0.5, 0.5, 0.5, 0.0,0.0,1.0,
        -0.5,-0.5,-0.5, 0.0,0.0,-1.0, -0.5, 0.5,-0.5, 0.0,0.0,-1.0,  0.5, 0.5,-0.5, 0.0,0.0,-1.0,
        -0.5,-0.5,-0.5, 0.0,0.0,-1.0,  0.5, 0.5,-0.5, 0.0,0.0,-1.0,  0.5,-0.5,-0.5, 0.0,0.0,-1.0,
        -0.5, 0.5,-0.5, 0.0,1.0,0.0, -0.5, 0.5, 0.5, 0.0,1.0,0.0,  0.5, 0.5, 0.5, 0.0,1.0,0.0,
        -0.5, 0.5,-0.5, 0.0,1.0,0.0,  0.5, 0.5, 0.5, 0.0,1.0,0.0,  0.5, 0.5,-0.5, 0.0,1.0,0.0,
        -0.5,-0.5,-0.5, 0.0,-1.0,0.0,  0.5,-0.5,-0.5, 0.0,-1.0,0.0,  0.5,-0.5, 0.5, 0.0,-1.0,0.0,
        -0.5,-0.5,-0.5, 0.0,-1.0,0.0,  0.5,-0.5, 0.5, 0.0,-1.0,0.0, -0.5,-0.5, 0.5, 0.0,-1.0,0.0,
         0.5,-0.5,-0.5, 1.0,0.0,0.0,  0.5, 0.5,-0.5, 1.0,0.0,0.0,  0.5, 0.5, 0.5, 1.0,0.0,0.0,
         0.5,-0.5,-0.5, 1.0,0.0,0.0,  0.5, 0.5, 0.5, 1.0,0.0,0.0,  0.5,-0.5, 0.5, 1.0,0.0,0.0,
        -0.5,-0.5,-0.5,-1.0,0.0,0.0, -0.5,-0.5, 0.5,-1.0,0.0,0.0, -0.5, 0.5, 0.5,-1.0,0.0,0.0,
        -0.5,-0.5,-0.5,-1.0,0.0,0.0, -0.5, 0.5, 0.5,-1.0,0.0,0.0, -0.5, 0.5,-0.5,-1.0,0.0,0.0,
    ];
    let vao = gl.create_vertex_array()?;
    let vbo = gl.create_buffer()?;
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(egui_glow::glow::ARRAY_BUFFER, Some(vbo));
    let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4);
    gl.buffer_data_u8_slice(egui_glow::glow::ARRAY_BUFFER, bytes, egui_glow::glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, egui_glow::glow::FLOAT, false, 24, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, egui_glow::glow::FLOAT, false, 24, 12);
    gl.bind_vertex_array(None);
    Ok((vao, vbo))
}

unsafe fn create_grid(gl: &egui_glow::glow::Context, half: i32) -> Result<(egui_glow::glow::VertexArray, egui_glow::glow::Buffer, i32), String> {
    use egui_glow::glow::HasContext;
    let mut v = Vec::new();
    let f = half as f32;
    for i in -half..=half {
        let p = i as f32;
        v.extend_from_slice(&[-f, 0.0, p, 0.0, 1.0, 0.0, f, 0.0, p, 0.0, 1.0, 0.0]);
        v.extend_from_slice(&[p, 0.0, -f, 0.0, 1.0, 0.0, p, 0.0, f, 0.0, 1.0, 0.0]);
    }
    let vao = gl.create_vertex_array()?;
    let vbo = gl.create_buffer()?;
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(egui_glow::glow::ARRAY_BUFFER, Some(vbo));
    let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4);
    gl.buffer_data_u8_slice(egui_glow::glow::ARRAY_BUFFER, bytes, egui_glow::glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, egui_glow::glow::FLOAT, false, 24, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, egui_glow::glow::FLOAT, false, 24, 12);
    gl.bind_vertex_array(None);
    Ok((vao, vbo, (v.len() / 6) as i32))
}

fn apply_blender_theme(ctx: &egui::Context) {
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
