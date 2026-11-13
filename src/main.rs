use eframe::egui;
use std::sync::{Arc, Mutex};

mod core {
    use glam::{EulerRot, Mat4, Vec3};

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum WorkspaceTab {
        Layout,
        Modeling,
        Animation,
        Shading,
        Compositing,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum InteractionMode {
        Object,
        Edit,
        Pose,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum PropertiesTab {
        Render,
        Object,
        Material,
        Bone,
    }

    #[derive(Clone, Debug)]
    pub struct Transform {
        pub position: Vec3,
        pub rotation_deg: Vec3,
        pub scale: Vec3,
    }

    impl Default for Transform {
        fn default() -> Self {
            Self {
                position: Vec3::ZERO,
                rotation_deg: Vec3::ZERO,
                scale: Vec3::ONE,
            }
        }
    }

    impl Transform {
        pub fn matrix(&self) -> Mat4 {
            let r = self.rotation_deg * std::f32::consts::PI / 180.0;
            Mat4::from_scale_rotation_translation(
                self.scale,
                glam::Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z),
                self.position,
            )
        }
    }

    #[derive(Clone, Debug)]
    pub struct SceneObject {
        pub id: usize,
        pub name: String,
        pub transform: Transform,
        pub visible: bool,
    }

    pub struct TimelineState {
        pub current_frame: i32,
        pub start_frame: i32,
        pub end_frame: i32,
        pub playing: bool,
    }

    impl Default for TimelineState {
        fn default() -> Self {
            Self {
                current_frame: 1,
                start_frame: 1,
                end_frame: 250,
                playing: false,
            }
        }
    }

    pub struct Scene {
        pub objects: Vec<SceneObject>,
        pub selected_id: Option<usize>,
        pub timeline: TimelineState,
    }

    impl Default for Scene {
        fn default() -> Self {
            Self {
                objects: vec![SceneObject {
                    id: 1,
                    name: "Steve_Cube".to_string(),
                    transform: Transform::default(),
                    visible: true,
                }],
                selected_id: Some(1),
                timeline: TimelineState::default(),
            }
        }
    }
}

mod render_api {
    use glam::{Mat4, Vec3};

    #[derive(Clone, Debug)]
    pub struct ViewportCamera {
        pub target: Vec3,
        pub distance: f32,
        pub yaw: f32,
        pub pitch: f32,
        pub fov_deg: f32,
    }

    impl Default for ViewportCamera {
        fn default() -> Self {
            Self {
                target: Vec3::ZERO,
                distance: 6.0,
                yaw: 0.78,
                pitch: 0.45,
                fov_deg: 45.0,
            }
        }
    }

    impl ViewportCamera {
        pub fn view_matrix(&self) -> Mat4 {
            let x = self.distance * self.pitch.cos() * self.yaw.sin();
            let y = self.distance * self.pitch.sin();
            let z = self.distance * self.pitch.cos() * self.yaw.cos();
            let eye = self.target + Vec3::new(x, y, z);
            Mat4::look_at_rh(eye, self.target, Vec3::Y)
        }

        pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
            Mat4::perspective_rh(self.fov_deg.to_radians(), aspect.max(0.01), 0.1, 500.0)
        }
    }
}

mod render_gl {
    use crate::core::Scene;
    use crate::render_api::ViewportCamera;
    use egui_glow::glow::{self, HasContext};
    use glam::Mat4;
    use std::sync::Arc;

    const VERT_SRC: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_color;
uniform mat4 u_mvp;
out vec3 v_color;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_color = a_color;
}
"#;

    const FRAG_SRC: &str = r#"#version 330 core
in vec3 v_color;
out vec4 FragColor;
void main() {
    FragColor = vec4(v_color, 1.0);
}
"#;

    pub struct GlViewportRenderer {
        gl: Arc<glow::Context>,
        program: glow::Program,
        cube_vao: glow::VertexArray,
        cube_vbo: glow::Buffer,
        cube_vert_count: i32,
        grid_vao: glow::VertexArray,
        grid_vbo: glow::Buffer,
        grid_vert_count: i32,
    }

    impl GlViewportRenderer {
        pub fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
            unsafe {
                let program = compile_program(&gl, VERT_SRC, FRAG_SRC)?;
                let (cube_vao, cube_vbo, cube_vert_count) = create_cube(&gl)?;
                let (grid_vao, grid_vbo, grid_vert_count) = create_grid(&gl, 10)?;
                Ok(Self {
                    gl,
                    program,
                    cube_vao,
                    cube_vbo,
                    cube_vert_count,
                    grid_vao,
                    grid_vbo,
                    grid_vert_count,
                })
            }
        }

        pub fn render(&self, scene: &Scene, camera: &ViewportCamera, vp_rect: [i32; 4]) {
            let [x, y, w, h] = vp_rect;
            if w <= 0 || h <= 0 {
                return;
            }
            let aspect = w as f32 / h as f32;
            let vp = camera.projection_matrix(aspect) * camera.view_matrix();

            unsafe {
                let gl = &self.gl;
                gl.viewport(x, y, w, h);
                gl.enable(glow::DEPTH_TEST);
                gl.depth_func(glow::LESS);
                gl.clear(glow::DEPTH_BUFFER_BIT);

                gl.use_program(Some(self.program));
                let mvp_loc = gl.get_uniform_location(self.program, "u_mvp");

                gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &vp.to_cols_array());
                gl.bind_vertex_array(Some(self.grid_vao));
                gl.draw_arrays(glow::LINES, 0, self.grid_vert_count);

                gl.bind_vertex_array(Some(self.cube_vao));
                for obj in &scene.objects {
                    if !obj.visible {
                        continue;
                    }
                    let mvp: Mat4 = vp * obj.transform.matrix();
                    gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mvp.to_cols_array());
                    gl.draw_arrays(glow::TRIANGLES, 0, self.cube_vert_count);
                }

                gl.bind_vertex_array(None);
                gl.use_program(None);
                gl.disable(glow::DEPTH_TEST);
            }
        }

        pub fn destroy(&self) {
            unsafe {
                self.gl.delete_program(self.program);
                self.gl.delete_vertex_array(self.cube_vao);
                self.gl.delete_buffer(self.cube_vbo);
                self.gl.delete_vertex_array(self.grid_vao);
                self.gl.delete_buffer(self.grid_vbo);
            }
        }
    }

    unsafe fn compile_program(
        gl: &glow::Context,
        vert: &str,
        frag: &str,
    ) -> Result<glow::Program, String> {
        let program = gl.create_program()?;
        let vs = gl.create_shader(glow::VERTEX_SHADER)?;
        gl.shader_source(vs, vert);
        gl.compile_shader(vs);
        if !gl.get_shader_compile_status(vs) {
            return Err(gl.get_shader_info_log(vs));
        }
        let fs = gl.create_shader(glow::FRAGMENT_SHADER)?;
        gl.shader_source(fs, frag);
        gl.compile_shader(fs);
        if !gl.get_shader_compile_status(fs) {
            return Err(gl.get_shader_info_log(fs));
        }
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        Ok(program)
    }

    unsafe fn upload_pos_color(
        gl: &glow::Context,
        data: &[f32],
    ) -> Result<(glow::VertexArray, glow::Buffer), String> {
        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let bytes = std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            std::mem::size_of_val(data),
        );
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
        let stride = 6 * std::mem::size_of::<f32>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.bind_vertex_array(None);
        Ok((vao, vbo))
    }

    unsafe fn create_cube(
        gl: &glow::Context,
    ) -> Result<(glow::VertexArray, glow::Buffer, i32), String> {
        #[rustfmt::skip]
        let v: [f32; 216] = [
            -0.5,-0.5, 0.5, 0.72,0.72,0.75,  0.5,-0.5, 0.5, 0.72,0.72,0.75,  0.5, 0.5, 0.5, 0.72,0.72,0.75,
            -0.5,-0.5, 0.5, 0.72,0.72,0.75,  0.5, 0.5, 0.5, 0.72,0.72,0.75, -0.5, 0.5, 0.5, 0.72,0.72,0.75,
            -0.5,-0.5,-0.5, 0.45,0.45,0.48, -0.5, 0.5,-0.5, 0.45,0.45,0.48,  0.5, 0.5,-0.5, 0.45,0.45,0.48,
            -0.5,-0.5,-0.5, 0.45,0.45,0.48,  0.5, 0.5,-0.5, 0.45,0.45,0.48,  0.5,-0.5,-0.5, 0.45,0.45,0.48,
            -0.5, 0.5,-0.5, 0.85,0.85,0.88, -0.5, 0.5, 0.5, 0.85,0.85,0.88,  0.5, 0.5, 0.5, 0.85,0.85,0.88,
            -0.5, 0.5,-0.5, 0.85,0.85,0.88,  0.5, 0.5, 0.5, 0.85,0.85,0.88,  0.5, 0.5,-0.5, 0.85,0.85,0.88,
            -0.5,-0.5,-0.5, 0.30,0.30,0.32,  0.5,-0.5,-0.5, 0.30,0.30,0.32,  0.5,-0.5, 0.5, 0.30,0.30,0.32,
            -0.5,-0.5,-0.5, 0.30,0.30,0.32,  0.5,-0.5, 0.5, 0.30,0.30,0.32, -0.5,-0.5, 0.5, 0.30,0.30,0.32,
             0.5,-0.5,-0.5, 0.60,0.60,0.63,  0.5, 0.5,-0.5, 0.60,0.60,0.63,  0.5, 0.5, 0.5, 0.60,0.60,0.63,
             0.5,-0.5,-0.5, 0.60,0.60,0.63,  0.5, 0.5, 0.5, 0.60,0.60,0.63,  0.5,-0.5, 0.5, 0.60,0.60,0.63,
            -0.5,-0.5,-0.5, 0.52,0.52,0.55, -0.5,-0.5, 0.5, 0.52,0.52,0.55, -0.5, 0.5, 0.5, 0.52,0.52,0.55,
            -0.5,-0.5,-0.5, 0.52,0.52,0.55, -0.5, 0.5, 0.5, 0.52,0.52,0.55, -0.5, 0.5,-0.5, 0.52,0.52,0.55,
        ];
        let (vao, vbo) = upload_pos_color(gl, &v)?;
        Ok((vao, vbo, 36))
    }

    unsafe fn create_grid(
        gl: &glow::Context,
        half: i32,
    ) -> Result<(glow::VertexArray, glow::Buffer, i32), String> {
        let mut v = Vec::new();
        let f = half as f32;
        for i in -half..=half {
            let p = i as f32;
            if i == 0 {
                v.extend_from_slice(&[-f, 0.0, 0.0, 0.75, 0.25, 0.25, f, 0.0, 0.0, 0.75, 0.25, 0.25]);
                v.extend_from_slice(&[0.0, 0.0, -f, 0.25, 0.75, 0.25, 0.0, 0.0, f, 0.25, 0.75, 0.25]);
            } else {
                v.extend_from_slice(&[-f, 0.0, p, 0.30, 0.30, 0.30, f, 0.0, p, 0.30, 0.30, 0.30]);
                v.extend_from_slice(&[p, 0.0, -f, 0.30, 0.30, 0.30, p, 0.0, f, 0.30, 0.30, 0.30]);
            }
        }
        let count = (v.len() / 6) as i32;
        let (vao, vbo) = upload_pos_color(gl, &v)?;
        Ok((vao, vbo, count))
    }
}

use core::{InteractionMode, PropertiesTab, Scene, SceneObject, Transform, WorkspaceTab};
use render_api::ViewportCamera;
use render_gl::GlViewportRenderer;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("APPCRAFT 0.1"),
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
    scene: Scene,
    camera: ViewportCamera,
    workspace: WorkspaceTab,
    mode: InteractionMode,
    prop_tab: PropertiesTab,
    renderer: Arc<Mutex<Option<GlViewportRenderer>>>,
    next_id: usize,
}

impl AppCraftApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_blender_theme(&cc.egui_ctx);
        let renderer = cc
            .gl
            .as_ref()
            .and_then(|gl| GlViewportRenderer::new(Arc::clone(gl)).ok());
        Self {
            scene: Scene::default(),
            camera: ViewportCamera::default(),
            workspace: WorkspaceTab::Layout,
            mode: InteractionMode::Object,
            prop_tab: PropertiesTab::Object,
            renderer: Arc::new(Mutex::new(renderer)),
            next_id: 2,
        }
    }
}

impl eframe::App for AppCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !ctx.wants_keyboard_input() {
            self.scene.timeline.playing = !self.scene.timeline.playing;
        }

        if self.scene.timeline.playing {
            self.scene.timeline.current_frame += 1;
            if self.scene.timeline.current_frame > self.scene.timeline.end_frame {
                self.scene.timeline.current_frame = self.scene.timeline.start_frame;
            }
            if let Some(id) = self.scene.selected_id {
                if let Some(obj) = self.scene.objects.iter_mut().find(|o| o.id == id) {
                    obj.transform.rotation_deg.y =
                        (obj.transform.rotation_deg.y + 1.5) % 360.0;
                }
            }
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("APPCRAFT");
                    ui.separator();
                    ui.menu_button("File", |ui| {
                        if ui.button("New Scene").clicked() {
                            self.scene = Scene::default();
                            ui.close_menu();
                        }
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Add", |ui| {
                        if ui.button("Cube").clicked() {
                            let id = self.next_id;
                            self.next_id += 1;
                            self.scene.objects.push(SceneObject {
                                id,
                                name: format!("Cube.{:03}", id),
                                transform: Transform::default(),
                                visible: true,
                            });
                            self.scene.selected_id = Some(id);
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    for (tab, label) in [
                        (WorkspaceTab::Layout, "Layout"),
                        (WorkspaceTab::Modeling, "Modeling"),
                        (WorkspaceTab::Animation, "Animation"),
                        (WorkspaceTab::Shading, "Shading"),
                        (WorkspaceTab::Compositing, "Compositing"),
                    ] {
                        if ui.selectable_label(self.workspace == tab, label).clicked() {
                            self.workspace = tab;
                        }
                    }
                });
            });

        egui::TopBottomPanel::bottom("timeline")
            .default_height(110.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let play_txt = if self.scene.timeline.playing {
                        "Pause"
                    } else {
                        "Play"
                    };
                    if ui.button(play_txt).clicked() {
                        self.scene.timeline.playing = !self.scene.timeline.playing;
                    }
                    ui.separator();
                    ui.label("Frame:");
                    ui.add(
                        egui::DragValue::new(&mut self.scene.timeline.current_frame)
                            .range(self.scene.timeline.start_frame..=self.scene.timeline.end_frame),
                    );
                });
                ui.add(
                    egui::Slider::new(
                        &mut self.scene.timeline.current_frame,
                        self.scene.timeline.start_frame..=self.scene.timeline.end_frame,
                    )
                    .show_value(false),
                );
            });

        egui::SidePanel::right("right_dock")
            .default_width(270.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Outliner");
                egui::ScrollArea::vertical()
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for obj in &mut self.scene.objects {
                            let selected = self.scene.selected_id == Some(obj.id);
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut obj.visible, "");
                                if ui.selectable_label(selected, &obj.name).clicked() {
                                    self.scene.selected_id = Some(obj.id);
                                }
                            });
                        }
                    });

                ui.separator();
                ui.heading("Properties");
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (PropertiesTab::Render, "Render"),
                        (PropertiesTab::Object, "Object"),
                        (PropertiesTab::Material, "Material"),
                        (PropertiesTab::Bone, "Bone"),
                    ] {
                        if ui.selectable_label(self.prop_tab == tab, label).clicked() {
                            self.prop_tab = tab;
                        }
                    }
                });
                ui.separator();

                match self.prop_tab {
                    PropertiesTab::Object => {
                        if let Some(id) = self.scene.selected_id {
                            if let Some(obj) =
                                self.scene.objects.iter_mut().find(|o| o.id == id)
                            {
                                ui.label(&obj.name);
                                draw_vec3(ui, "Location", &mut obj.transform.position, 0.05);
                                draw_vec3(ui, "Rotation", &mut obj.transform.rotation_deg, 1.0);
                                draw_vec3(ui, "Scale", &mut obj.transform.scale, 0.05);
                            }
                        }
                    }
                    PropertiesTab::Render => {
                        ui.label("Engine: APPCRAFT GL");
                        ui.label("Target: OpenGL 3.3 Core");
                    }
                    PropertiesTab::Material => {
                        ui.label("Standard PBR Contract");
                    }
                    PropertiesTab::Bone => {
                        ui.label("Armature & IK System");
                    }
                }
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("mode_combo")
                        .selected_text(format!("{:?} Mode", self.mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.mode, InteractionMode::Object, "Object Mode");
                            ui.selectable_value(&mut self.mode, InteractionMode::Edit, "Edit Mode");
                            ui.selectable_value(&mut self.mode, InteractionMode::Pose, "Pose Mode");
                        });
                    ui.label("Orbit: Middle/Right Drag | Zoom: Scroll | Space: Play");
                });

                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(57, 57, 57));

                let response = ui.allocate_rect(rect, egui::Sense::drag());
                if response.dragged_by(egui::PointerButton::Middle)
                    || response.dragged_by(egui::PointerButton::Secondary)
                {
                    let delta = response.drag_delta();
                    self.camera.yaw -= delta.x * 0.01;
                    self.camera.pitch = (self.camera.pitch + delta.y * 0.01).clamp(-1.45, 1.45);
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        self.camera.distance = (self.camera.distance - scroll * 0.01).clamp(1.5, 100.0);
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

                let renderer_arc = Arc::clone(&self.renderer);
                let scene_objects = self.scene.objects.clone();
                let camera_clone = self.camera.clone();

                let cb = egui::PaintCallback {
                    rect,
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                        move |_info, _painter| {
                            if let Ok(guard) = renderer_arc.lock() {
                                if let Some(r) = guard.as_ref() {
                                    let tmp_scene = Scene {
                                        objects: scene_objects.clone(),
                                        selected_id: None,
                                        timeline: Default::default(),
                                    };
                                    r.render(&tmp_scene, &camera_clone, vp_rect);
                                }
                            }
                        },
                    )),
                };
                ui.painter().add(cb);
            });
    }

    fn on_exit(&mut self, _gl: Option<&egui_glow::glow::Context>) {
        if let Ok(guard) = self.renderer.lock() {
            if let Some(r) = guard.as_ref() {
                r.destroy();
            }
        }
    }
}

fn draw_vec3(ui: &mut egui::Ui, label: &str, v: &mut glam::Vec3, speed: f32) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut v.x).speed(speed).prefix("X: "));
        ui.add(egui::DragValue::new(&mut v.y).speed(speed).prefix("Y: "));
        ui.add(egui::DragValue::new(&mut v.z).speed(speed).prefix("Z: "));
    });
}

fn apply_blender_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_rgb(48, 48, 48);
    style.visuals.window_fill = egui::Color32::from_rgb(48, 48, 48);
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(61, 61, 61);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(84, 84, 84);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(101, 101, 101);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(71, 114, 179);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(71, 114, 179);
    ctx.set_style(style);
  }
