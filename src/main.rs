use eframe::egui;
use egui_glow::glow::{self, HasContext};
use glam::{EulerRot, Mat4, Quat, Vec3};
use std::sync::{Arc, Mutex};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("APPCRAFT - Minecraft Armature & Animation Core"),
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

// ------------------- BONE & ARMATURE SYSTEM -------------------
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub head: Vec3,
    pub tail: Vec3,
    pub pose_rot: Vec3, // Rotation in degrees (X, Y, Z)
}

#[derive(Clone, Debug)]
pub struct Armature {
    pub bones: Vec<Bone>,
}

impl Armature {
    pub fn new_steve() -> Self {
        let mut b = Vec::new();

        // 0: Root / Hips
        b.push(Bone { name: "Root".into(), parent: None, head: Vec3::new(0.0, 0.75, 0.0), tail: Vec3::new(0.0, 1.125, 0.0), pose_rot: Vec3::ZERO });
        // 1: Torso
        b.push(Bone { name: "Torso".into(), parent: Some(0), head: Vec3::new(0.0, 1.125, 0.0), tail: Vec3::new(0.0, 1.5, 0.0), pose_rot: Vec3::ZERO });
        // 2: Head
        b.push(Bone { name: "Head".into(), parent: Some(1), head: Vec3::new(0.0, 1.5, 0.0), tail: Vec3::new(0.0, 2.0, 0.0), pose_rot: Vec3::ZERO });

        // Right Arm (Shoulder -> Elbow -> Hand)
        b.push(Bone { name: "Arm_Upper_R".into(), parent: Some(1), head: Vec3::new(0.375, 1.375, 0.0), tail: Vec3::new(0.375, 1.0, 0.0), pose_rot: Vec3::ZERO });
        b.push(Bone { name: "Arm_Lower_R (Elbow)".into(), parent: Some(3), head: Vec3::new(0.375, 1.0, 0.0), tail: Vec3::new(0.375, 0.625, 0.0), pose_rot: Vec3::ZERO });

        // Left Arm (Shoulder -> Elbow -> Hand)
        b.push(Bone { name: "Arm_Upper_L".into(), parent: Some(1), head: Vec3::new(-0.375, 1.375, 0.0), tail: Vec3::new(-0.375, 1.0, 0.0), pose_rot: Vec3::ZERO });
        b.push(Bone { name: "Arm_Lower_L (Elbow)".into(), parent: Some(5), head: Vec3::new(-0.375, 1.0, 0.0), tail: Vec3::new(-0.375, 0.625, 0.0), pose_rot: Vec3::ZERO });

        // Right Leg (Hip -> Knee -> Foot)
        b.push(Bone { name: "Leg_Upper_R".into(), parent: Some(0), head: Vec3::new(0.125, 0.75, 0.0), tail: Vec3::new(0.125, 0.375, 0.0), pose_rot: Vec3::ZERO });
        b.push(Bone { name: "Leg_Lower_R (Knee)".into(), parent: Some(7), head: Vec3::new(0.125, 0.375, 0.0), tail: Vec3::new(0.125, 0.0, 0.0), pose_rot: Vec3::ZERO });

        // Left Leg (Hip -> Knee -> Foot)
        b.push(Bone { name: "Leg_Upper_L".into(), parent: Some(0), head: Vec3::new(-0.125, 0.75, 0.0), tail: Vec3::new(-0.125, 0.375, 0.0), pose_rot: Vec3::ZERO });
        b.push(Bone { name: "Leg_Lower_L (Knee)".into(), parent: Some(9), head: Vec3::new(-0.125, 0.375, 0.0), tail: Vec3::new(-0.125, 0.0, 0.0), pose_rot: Vec3::ZERO });

        Self { bones: b }
    }

    pub fn compute_matrices(&self) -> Vec<Mat4> {
        let count = self.bones.len();
        let mut mats = vec![Mat4::IDENTITY; count];

        for i in 0..count {
            let b = &self.bones[i];
            let r = b.pose_rot * std::f32::consts::PI / 180.0;
            let q = Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z);

            // Pivot at head
            let local_pose = Mat4::from_translation(b.head)
                * Mat4::from_quat(q)
                * Mat4::from_translation(-b.head);

            mats[i] = if let Some(parent_idx) = b.parent {
                mats[parent_idx] * local_pose
            } else {
                local_pose
            };
        }
        mats
    }

    pub fn reset_pose(&mut self) {
        for b in &mut self.bones {
            b.pose_rot = Vec3::ZERO;
        }
    }
}

// ------------------- MESH LIMB PIECES -------------------
#[derive(Clone)]
pub struct LimbPiece {
    pub bone_idx: usize,
    pub offset: Vec3,
    pub size: Vec3,
    pub color: [f32; 3],
}

// ------------------- KEYFRAMING SYSTEM -------------------
#[derive(Clone)]
pub struct Keyframe {
    pub frame: i32,
    pub rotations: Vec<Vec3>,
}

// ------------------- APPLICATION STATE -------------------
struct AppCraftApp {
    armature: Armature,
    limbs: Vec<LimbPiece>,
    selected_bone: usize,
    show_bones_xray: bool,
    show_mesh: bool,

    // Timeline
    current_frame: i32,
    playing: bool,
    keyframes: Vec<Keyframe>,

    // Camera
    cam_target: Vec3,
    cam_dist: f32,
    cam_yaw: f32,
    cam_pitch: f32,

    gl_renderer: Arc<Mutex<Option<GlRenderer>>>,
    status: String,
}

impl AppCraftApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_blender_dark(&cc.egui_ctx);
        let gl_renderer = cc.gl.as_ref().and_then(|gl| GlRenderer::new(Arc::clone(gl)).ok());

        let limbs = vec![
            // Head
            LimbPiece { bone_idx: 2, offset: Vec3::new(0.0, 0.25, 0.0), size: Vec3::new(0.5, 0.5, 0.5), color: [0.85, 0.65, 0.52] },
            // Torso
            LimbPiece { bone_idx: 1, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.5, 0.75, 0.25), color: [0.18, 0.62, 0.72] },
            // Right Arm (Upper & Lower)
            LimbPiece { bone_idx: 3, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.18, 0.62, 0.72] },
            LimbPiece { bone_idx: 4, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.85, 0.65, 0.52] },
            // Left Arm (Upper & Lower)
            LimbPiece { bone_idx: 5, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.18, 0.62, 0.72] },
            LimbPiece { bone_idx: 6, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.85, 0.65, 0.52] },
            // Right Leg (Upper & Lower)
            LimbPiece { bone_idx: 7, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.22, 0.25, 0.65] },
            LimbPiece { bone_idx: 8, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.22, 0.25, 0.65] },
            // Left Leg (Upper & Lower)
            LimbPiece { bone_idx: 9, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.22, 0.25, 0.65] },
            LimbPiece { bone_idx: 10, offset: Vec3::new(0.0, -0.1875, 0.0), size: Vec3::new(0.25, 0.375, 0.25), color: [0.22, 0.25, 0.65] },
        ];

        Self {
            armature: Armature::new_steve(),
            limbs,
            selected_bone: 4, // Start with right elbow
            show_bones_xray: true,
            show_mesh: true,
            current_frame: 1,
            playing: false,
            keyframes: Vec::new(),
            cam_target: Vec3::new(0.0, 1.0, 0.0),
            cam_dist: 5.0,
            cam_yaw: 0.75,
            cam_pitch: 0.35,
            gl_renderer: Arc::new(Mutex::new(gl_renderer)),
            status: "Pose Mode Active | Space: Play | I: Keyframe | Alt+R: Reset".into(),
        }
    }

    fn insert_keyframe(&mut self) {
        let rots: Vec<Vec3> = self.armature.bones.iter().map(|b| b.pose_rot).collect();
        let f = self.current_frame;
        if let Some(existing) = self.keyframes.iter_mut().find(|k| k.frame == f) {
            existing.rotations = rots;
        } else {
            self.keyframes.push(Keyframe { frame: f, rotations: rots });
            self.keyframes.sort_by_key(|k| k.frame);
        }
        self.status = format!("Saved Keyframe at Frame {}", f);
    }

    fn evaluate_timeline(&mut self) {
        if self.keyframes.is_empty() { return; }
        let f = self.current_frame;

        if f <= self.keyframes[0].frame {
            let r = &self.keyframes[0].rotations;
            for (i, b) in self.armature.bones.iter_mut().enumerate() { b.pose_rot = r[i]; }
            return;
        }
        let last = &self.keyframes[self.keyframes.len() - 1];
        if f >= last.frame {
            for (i, b) in self.armature.bones.iter_mut().enumerate() { b.pose_rot = last.rotations[i]; }
            return;
        }

        for i in 0..self.keyframes.len() - 1 {
            let k0 = &self.keyframes[i];
            let k1 = &self.keyframes[i + 1];
            if f >= k0.frame && f <= k1.frame {
                let span = (k1.frame - k0.frame).max(1) as f32;
                let t = (f - k0.frame) as f32 / span;
                let smooth = t * t * (3.0 - 2.0 * t);
                for (b_idx, b) in self.armature.bones.iter_mut().enumerate() {
                    b.pose_rot = k0.rotations[b_idx].lerp(k1.rotations[b_idx], smooth);
                }
                break;
            }
        }
    }

    fn load_walk_preset(&mut self) {
        self.keyframes.clear();
        self.armature.reset_pose();

        // Frame 1: Right leg forward, Left leg back, arms opposite
        self.armature.bones[7].pose_rot.x = 35.0;  // Thigh R forward
        self.armature.bones[8].pose_rot.x = -25.0; // Knee R bent
        self.armature.bones[9].pose_rot.x = -35.0; // Thigh L back
        self.armature.bones[10].pose_rot.x = 0.0;
        self.armature.bones[3].pose_rot.x = -35.0; // Arm R back
        self.armature.bones[4].pose_rot.x = 40.0;  // Elbow R bent
        self.armature.bones[5].pose_rot.x = 35.0;  // Arm L forward
        self.armature.bones[6].pose_rot.x = 30.0;  // Elbow L bent
        self.current_frame = 1;
        self.insert_keyframe();

        // Frame 15: Inverted
        self.armature.bones[7].pose_rot.x = -35.0;
        self.armature.bones[8].pose_rot.x = 0.0;
        self.armature.bones[9].pose_rot.x = 35.0;
        self.armature.bones[10].pose_rot.x = -25.0;
        self.armature.bones[3].pose_rot.x = 35.0;
        self.armature.bones[4].pose_rot.x = 30.0;
        self.armature.bones[5].pose_rot.x = -35.0;
        self.armature.bones[6].pose_rot.x = 40.0;
        self.current_frame = 15;
        self.insert_keyframe();

        // Frame 30: Loop back to Frame 1
        self.armature.bones[7].pose_rot.x = 35.0;
        self.armature.bones[8].pose_rot.x = -25.0;
        self.armature.bones[9].pose_rot.x = -35.0;
        self.armature.bones[10].pose_rot.x = 0.0;
        self.armature.bones[3].pose_rot.x = -35.0;
        self.armature.bones[4].pose_rot.x = 40.0;
        self.armature.bones[5].pose_rot.x = 35.0;
        self.armature.bones[6].pose_rot.x = 30.0;
        self.current_frame = 30;
        self.insert_keyframe();

        self.current_frame = 1;
        self.playing = true;
        self.status = "Loaded Walk Cycle Animation Preset!".into();
    }
}

impl eframe::App for AppCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_blender_dark(ctx);

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) { self.playing = !self.playing; }
                if i.key_pressed(egui::Key::I) { self.insert_keyframe(); }
                if i.modifiers.alt && i.key_pressed(egui::Key::R) {
                    self.armature.reset_pose();
                    self.status = "Reset Pose (Alt+R)".into();
                }
            });
        }

        if self.playing {
            self.current_frame += 1;
            if self.current_frame > 30 { self.current_frame = 1; }
            self.evaluate_timeline();
            ctx.request_repaint();
        }

        // Top Menu Bar
        egui::TopBottomPanel::top("top_panel").exact_height(28.0).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label("APPCRAFT Core");
                ui.separator();
                ui.checkbox(&mut self.show_bones_xray, "Show Blender Armature (X-Ray)");
                ui.checkbox(&mut self.show_mesh, "Show Minecraft Steve");
                ui.separator();
                if ui.button("Walk Animation Preset").clicked() {
                    self.load_walk_preset();
                }
                if ui.button("Reset Pose (Alt+R)").clicked() {
                    self.armature.reset_pose();
                }
            });
        });

        // Bottom Timeline Panel
        egui::TopBottomPanel::bottom("bottom_timeline").exact_height(75.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let play_txt = if self.playing { "Pause (Space)" } else { "Play (Space)" };
                if ui.button(play_txt).clicked() { self.playing = !self.playing; }
                if ui.button("Insert Keyframe (I)").clicked() { self.insert_keyframe(); }
                if ui.button("Clear Keys").clicked() { self.keyframes.clear(); }
                ui.separator();
                ui.label(format!("Frame: {}", self.current_frame));
                ui.label(format!("(Saved Keys: {})", self.keyframes.len()));
            });
            let mut f = self.current_frame;
            if ui.add(egui::Slider::new(&mut f, 1..=60).show_value(false)).changed() {
                self.current_frame = f;
                self.evaluate_timeline();
            }
            ui.label(&self.status);
        });

        // Left Panel: Armature Hierarchy Tree
        egui::SidePanel::left("left_tree").default_width(220.0).show(ctx, |ui| {
            ui.heading("Armature Bones");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for i in 0..self.armature.bones.len() {
                    let b = &self.armature.bones[i];
                    let prefix = if b.parent.is_some() { "  |- " } else { "" };
                    let sel = self.selected_bone == i;
                    if ui.selectable_label(sel, format!("{}{}", prefix, b.name)).clicked() {
                        self.selected_bone = i;
                    }
                }
            });
        });

        // Right Panel: Bone Pose & Bending Controls
        egui::SidePanel::right("right_controls").default_width(280.0).show(ctx, |ui| {
            ui.heading("Bone Pose Controls");
            ui.separator();
            let b = &mut self.armature.bones[self.selected_bone];
            ui.label(format!("Selected: {}", b.name));

            ui.add(egui::Slider::new(&mut b.pose_rot.x, -180.0..=180.0).text("Bend X (Pitch)"));
            ui.add(egui::Slider::new(&mut b.pose_rot.y, -180.0..=180.0).text("Twist Y (Yaw)"));
            ui.add(egui::Slider::new(&mut b.pose_rot.z, -180.0..=180.0).text("Roll Z"));

            ui.separator();
            ui.label("Quick Joint Actions:");
            if ui.button("Bend Joint (+45°)").clicked() { b.pose_rot.x += 45.0; }
            if ui.button("Bend Joint (-45°)").clicked() { b.pose_rot.x -= 45.0; }
            if ui.button("Zero Bone Rotation").clicked() { b.pose_rot = Vec3::ZERO; }
        });

        // Central 3D Viewport
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, egui::Sense::drag());

            if response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary) {
                let d = response.drag_delta();
                self.cam_yaw -= d.x * 0.01;
                self.cam_pitch = (self.cam_pitch + d.y * 0.01).clamp(-1.45, 1.45);
            }
            if response.hovered() {
                let s = ctx.input(|i| i.raw_scroll_delta.y);
                if s.abs() > 0.0 {
                    self.cam_dist = (self.cam_dist - s * 0.01).clamp(1.5, 30.0);
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

            // Compute hierarchical bone matrices
            let bone_world_mats = self.armature.compute_matrices();

            let gl_renderer = Arc::clone(&self.gl_renderer);
            let limbs = self.limbs.clone();
            let bones = self.armature.bones.clone();
            let sel_bone = self.selected_bone;
            let show_mesh = self.show_mesh;
            let show_xray = self.show_bones_xray;

            let cb = egui::PaintCallback {
                rect,
                callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |_info, _painter| {
                    if let Ok(guard) = gl_renderer.lock() {
                        if let Some(r) = guard.as_ref() {
                            r.render(vp_rect, &vp, &limbs, &bones, &bone_world_mats, sel_bone, show_mesh, show_xray);
                        }
                    }
                })),
            };
            ui.painter().add(cb);
        });
    }
}

// ------------------- OPENGL 3.3 HARDWARE RENDERER -------------------
struct GlRenderer {
    gl: Arc<glow::Context>,
    prog: glow::Program,
    cube_vao: glow::VertexArray,
    cube_vbo: glow::Buffer,
    dyn_vao: glow::VertexArray,
    dyn_vbo: glow::Buffer,
}

impl GlRenderer {
    fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
        unsafe {
            let vs = "#version 330 core\nlayout(location=0) in vec3 pos; layout(location=1) in vec3 norm; uniform mat4 mvp; uniform mat4 model; uniform int unlit; out vec3 n; out vec3 p; void main(){ gl_Position=mvp*vec4(pos,1.0); n=mat3(model)*norm; p=norm; }";
            let fs = "#version 330 core\nin vec3 n; in vec3 p; uniform vec3 col; uniform int unlit; out vec4 fc; void main(){ if(unlit==1){ fc=vec4(p,1.0); return; } vec3 sun=normalize(vec3(0.5,0.85,0.4)); float d=max(dot(normalize(n),sun),0.0); vec3 c=col*(0.35+0.65*d); fc=vec4(c,1.0); }";

            let prog = compile_shader(&gl, vs, fs)?;
            let (cube_vao, cube_vbo) = create_cube(&gl)?;
            let dyn_vao = gl.create_vertex_array()?;
            let dyn_vbo = gl.create_buffer()?;

            Ok(Self { gl, prog, cube_vao, cube_vbo, dyn_vao, dyn_vbo })
        }
    }

    fn render(
        &self,
        vp_rect: [i32; 4],
        vp: &Mat4,
        limbs: &[LimbPiece],
        bones: &[Bone],
        bone_mats: &[Mat4],
        selected_bone: usize,
        show_mesh: bool,
        show_xray: bool,
    ) {
        let [x, y, w, h] = vp_rect;
        if w <= 0 || h <= 0 { return; }

        unsafe {
            let gl = &self.gl;
            gl.viewport(x, y, w, h);
            gl.enable(glow::DEPTH_TEST);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.prog));
            let mvp_loc = gl.get_uniform_location(self.prog, "mvp");
            let model_loc = gl.get_uniform_location(self.prog, "model");
            let col_loc = gl.get_uniform_location(self.prog, "col");
            let unlit_loc = gl.get_uniform_location(self.prog, "unlit");

            // 1. Draw Grid
            gl.uniform_1_i32(unlit_loc.as_ref(), 1);
            let mut grid_lines = Vec::new();
            for i in -10..=10 {
                let p = i as f32;
                grid_lines.extend_from_slice(&[-10.0, 0.0, p, 0.28, 0.28, 0.28, 10.0, 0.0, p, 0.28, 0.28, 0.28]);
                grid_lines.extend_from_slice(&[p, 0.0, -10.0, 0.28, 0.28, 0.28, p, 0.0, 10.0, 0.28, 0.28, 0.28]);
            }
            self.upload_and_draw_lines(&grid_lines, vp);

            // 2. Draw Minecraft Steve Mesh Parts
            if show_mesh {
                gl.uniform_1_i32(unlit_loc.as_ref(), 0);
                gl.bind_vertex_array(Some(self.cube_vao));

                for limb in limbs {
                    let bone_mat = bone_mats[limb.bone_idx];
                    let model = bone_mat * Mat4::from_translation(limb.offset) * Mat4::from_scale(limb.size);
                    let mvp = *vp * model;

                    gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mvp.to_cols_array());
                    gl.uniform_matrix_4_f32_slice(model_loc.as_ref(), false, &model.to_cols_array());
                    gl.uniform_3_f32(col_loc.as_ref(), limb.color[0], limb.color[1], limb.color[2]);
                    gl.draw_arrays(glow::TRIANGLES, 0, 36);
                }
            }

            // 3. Draw Blender Octahedral Armature Bones (X-Ray)
            if show_xray {
                gl.disable(glow::DEPTH_TEST);
                gl.uniform_1_i32(unlit_loc.as_ref(), 1);

                let mut bone_lines = Vec::new();
                for (i, b) in bones.iter().enumerate() {
                    let m = bone_mats[i];
                    let head_w = m.transform_point3(b.head);
                    let tail_w = m.transform_point3(b.tail);
                    let is_sel = i == selected_bone;
                    let col = if is_sel { Vec3::new(1.0, 0.55, 0.0) } else { Vec3::new(0.2, 0.75, 1.0) };

                    // Generate diamond octahedral bone shape
                    let b_vec = tail_w - head_w;
                    let b_len = b_vec.length().max(0.01);
                    let dir = b_vec / b_len;
                    let up = if dir.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
                    let side = dir.cross(up).normalize() * 0.055;
                    let fwd = side.cross(dir).normalize() * 0.055;
                    let mid = head_w + dir * (b_len * 0.25);

                    let p0 = mid + side;
                    let p1 = mid + fwd;
                    let p2 = mid - side;
                    let p3 = mid - fwd;

                    for &(v0, v1) in &[
                        (head_w, p0), (head_w, p1), (head_w, p2), (head_w, p3),
                        (p0, p1), (p1, p2), (p2, p3), (p3, p0),
                        (p0, tail_w), (p1, tail_w), (p2, tail_w), (p3, tail_w),
                    ] {
                        bone_lines.extend_from_slice(&[v0.x, v0.y, v0.z, col.x, col.y, col.z]);
                        bone_lines.extend_from_slice(&[v1.x, v1.y, v1.z, col.x, col.y, col.z]);
                    }
                }

                self.upload_and_draw_lines(&bone_lines, vp);
                gl.enable(glow::DEPTH_TEST);
            }

            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }

    unsafe fn upload_and_draw_lines(&self, lines: &[f32], vp: &Mat4) {
        if lines.is_empty() { return; }
        let gl = &self.gl;
        let mvp_loc = gl.get_uniform_location(self.prog, "mvp");
        let model_loc = gl.get_uniform_location(self.prog, "model");

        gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &vp.to_cols_array());
        gl.uniform_matrix_4_f32_slice(model_loc.as_ref(), false, &Mat4::IDENTITY.to_cols_array());

        gl.bind_vertex_array(Some(self.dyn_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.dyn_vbo));
        let bytes = std::slice::from_raw_parts(lines.as_ptr() as *const u8, lines.len() * 4);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);

        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);

        gl.draw_arrays(glow::LINES, 0, (lines.len() / 6) as i32);
    }
}

unsafe fn compile_shader(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    let p = gl.create_program()?;
    let v = gl.create_shader(glow::VERTEX_SHADER)?;
    gl.shader_source(v, vs);
    gl.compile_shader(v);
    let f = gl.create_shader(glow::FRAGMENT_SHADER)?;
    gl.shader_source(f, fs);
    gl.compile_shader(f);
    gl.attach_shader(p, v);
    gl.attach_shader(p, f);
    gl.link_program(p);
    Ok(p)
}

unsafe fn create_cube(gl: &glow::Context) -> Result<(glow::VertexArray, glow::Buffer), String> {
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
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4);
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);
    gl.bind_vertex_array(None);
    Ok((vao, vbo))
}

fn apply_blender_dark(ctx: &egui::Context) {
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
