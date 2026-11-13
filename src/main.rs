use eframe::egui;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

mod core {
    use glam::{EulerRot, Mat4, Vec3};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

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

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Transform {
        pub position: [f32; 3],
        pub rotation_deg: [f32; 3],
        pub scale: [f32; 3],
    }

    impl Default for Transform {
        fn default() -> Self {
            Self {
                position: [0.0, 0.0, 0.0],
                rotation_deg: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
            }
        }
    }

    impl Transform {
        pub fn local_matrix(&self) -> Mat4 {
            let p = Vec3::from_array(self.position);
            let r = Vec3::from_array(self.rotation_deg) * std::f32::consts::PI / 180.0;
            let s = Vec3::from_array(self.scale);
            Mat4::from_scale_rotation_translation(
                s,
                glam::Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z),
                p,
            )
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Keyframe {
        pub frame: i32,
        pub position: [f32; 3],
        pub rotation_deg: [f32; 3],
        pub scale: [f32; 3],
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct UnifiedAnimFile {
        pub version: u32,
        pub fps: f32,
        pub tracks: HashMap<String, Vec<Keyframe>>,
    }

    #[derive(Clone, Debug)]
    pub struct SceneObject {
        pub id: usize,
        pub parent_id: Option<usize>,
        pub name: String,
        pub mesh_id: usize,
        pub color: [f32; 3],
        pub transform: Transform,
        pub pivot_offset: [f32; 3],
        pub keyframes: Vec<Keyframe>,
        pub visible: bool,
    }

    impl SceneObject {
        pub fn insert_keyframe(&mut self, frame: i32) {
            let kf = Keyframe {
                frame,
                position: self.transform.position,
                rotation_deg: self.transform.rotation_deg,
                scale: self.transform.scale,
            };
            if let Some(existing) = self.keyframes.iter_mut().find(|k| k.frame == frame) {
                *existing = kf;
            } else {
                self.keyframes.push(kf);
                self.keyframes.sort_by_key(|k| k.frame);
            }
        }

        pub fn evaluate_at(&mut self, frame: i32) {
            if self.keyframes.is_empty() {
                return;
            }
            if frame <= self.keyframes[0].frame {
                let k = &self.keyframes[0];
                self.transform.position = k.position;
                self.transform.rotation_deg = k.rotation_deg;
                self.transform.scale = k.scale;
                return;
            }
            let last = &self.keyframes[self.keyframes.len() - 1];
            if frame >= last.frame {
                self.transform.position = last.position;
                self.transform.rotation_deg = last.rotation_deg;
                self.transform.scale = last.scale;
                return;
            }
            for w in self.keyframes.windows(2) {
                let a = &w[0];
                let b = &w[1];
                if frame >= a.frame && frame <= b.frame {
                    let span = (b.frame - a.frame).max(1) as f32;
                    let t = (frame - a.frame) as f32 / span;
                    let smooth = t * t * (3.0 - 2.0 * t);
                    for i in 0..3 {
                        self.transform.position[i] =
                            a.position[i] + (b.position[i] - a.position[i]) * smooth;
                        self.transform.rotation_deg[i] =
                            a.rotation_deg[i] + (b.rotation_deg[i] - a.rotation_deg[i]) * smooth;
                        self.transform.scale[i] =
                            a.scale[i] + (b.scale[i] - a.scale[i]) * smooth;
                    }
                    break;
                }
            }
        }
    }

    pub struct TimelineState {
        pub current_frame: i32,
        pub start_frame: i32,
        pub end_frame: i32,
        pub fps: f32,
        pub playing: bool,
    }

    impl Default for TimelineState {
        fn default() -> Self {
            Self {
                current_frame: 1,
                start_frame: 1,
                end_frame: 250,
                fps: 24.0,
                playing: false,
            }
        }
    }

    pub struct Scene {
        pub objects: Vec<SceneObject>,
        pub selected_id: Option<usize>,
        pub timeline: TimelineState,
        pub next_id: usize,
    }

    impl Default for Scene {
        fn default() -> Self {
            let mut s = Self {
                objects: Vec::new(),
                selected_id: None,
                timeline: TimelineState::default(),
                next_id: 1,
            };
            s.spawn_steve_rig();
            s
        }
    }

    impl Scene {
        pub fn alloc_id(&mut self) -> usize {
            let id = self.next_id;
            self.next_id += 1;
            id
        }

        pub fn world_matrix(&self, id: usize) -> Mat4 {
            let mut chain = Vec::new();
            let mut cur = Some(id);
            let mut guard = 0;
            while let Some(cid) = cur {
                if guard > 16 {
                    break;
                }
                if let Some(obj) = self.objects.iter().find(|o| o.id == cid) {
                    chain.push(obj.transform.local_matrix());
                    cur = obj.parent_id;
                } else {
                    break;
                }
                guard += 1;
            }
            let mut m = Mat4::IDENTITY;
            for mat in chain.into_iter().rev() {
                m *= mat;
            }
            m
        }

        pub fn spawn_steve_rig(&mut self) {
            let root_id = self.alloc_id();
            self.objects.push(SceneObject {
                id: root_id,
                parent_id: None,
                name: format!("Steve_Body_{}", root_id),
                mesh_id: 0,
                color: [0.18, 0.62, 0.72],
                transform: Transform {
                    position: [0.0, 1.125, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [0.5, 0.75, 0.25],
                },
                pivot_offset: [0.0, 0.0, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            let head_id = self.alloc_id();
            self.objects.push(SceneObject {
                id: head_id,
                parent_id: Some(root_id),
                name: "Steve_Head".to_string(),
                mesh_id: 0,
                color: [0.85, 0.66, 0.52],
                transform: Transform {
                    position: [0.0, 0.85, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [1.0, 0.66, 2.0],
                },
                pivot_offset: [0.0, 0.5, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            let r_arm = self.alloc_id();
            self.objects.push(SceneObject {
                id: r_arm,
                parent_id: Some(root_id),
                name: "Steve_Arm_R".to_string(),
                mesh_id: 0,
                color: [0.85, 0.66, 0.52],
                transform: Transform {
                    position: [0.75, 0.45, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [0.5, 1.0, 1.0],
                },
                pivot_offset: [0.0, -0.5, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            let l_arm = self.alloc_id();
            self.objects.push(SceneObject {
                id: l_arm,
                parent_id: Some(root_id),
                name: "Steve_Arm_L".to_string(),
                mesh_id: 0,
                color: [0.85, 0.66, 0.52],
                transform: Transform {
                    position: [-0.75, 0.45, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [0.5, 1.0, 1.0],
                },
                pivot_offset: [0.0, -0.5, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            let r_leg = self.alloc_id();
            self.objects.push(SceneObject {
                id: r_leg,
                parent_id: Some(root_id),
                name: "Steve_Leg_R".to_string(),
                mesh_id: 0,
                color: [0.22, 0.25, 0.65],
                transform: Transform {
                    position: [0.25, -0.5, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [0.5, 1.0, 1.0],
                },
                pivot_offset: [0.0, -0.5, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            let l_leg = self.alloc_id();
            self.objects.push(SceneObject {
                id: l_leg,
                parent_id: Some(root_id),
                name: "Steve_Leg_L".to_string(),
                mesh_id: 0,
                color: [0.22, 0.25, 0.65],
                transform: Transform {
                    position: [-0.25, -0.5, 0.0],
                    rotation_deg: [0.0, 0.0, 0.0],
                    scale: [0.5, 1.0, 1.0],
                },
                pivot_offset: [0.0, -0.5, 0.0],
                keyframes: Vec::new(),
                visible: true,
            });

            self.selected_id = Some(root_id);
        }

        pub fn evaluate_frame(&mut self) {
            let f = self.timeline.current_frame;
            for obj in &mut self.objects {
                obj.evaluate_at(f);
            }
        }

        pub fn export_unified_anim(&self) -> UnifiedAnimFile {
            let mut tracks = HashMap::new();
            for obj in &self.objects {
                if !obj.keyframes.is_empty() {
                    tracks.insert(obj.name.clone(), obj.keyframes.clone());
                }
            }
            UnifiedAnimFile {
                version: 1,
                fps: self.timeline.fps,
                tracks,
            }
        }

        pub fn import_unified_anim(&mut self, anim: UnifiedAnimFile) {
            self.timeline.fps = anim.fps;
            for obj in &mut self.objects {
                if let Some(kfs) = anim.tracks.get(&obj.name) {
                    obj.keyframes = kfs.clone();
                    obj.keyframes.sort_by_key(|k| k.frame);
                }
            }
            self.evaluate_frame();
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
                target: Vec3::new(0.0, 1.0, 0.0),
                distance: 6.5,
                yaw: 0.65,
                pitch: 0.35,
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
    use glam::{Mat4, Vec3};
    use std::collections::HashMap;
    use std::sync::Arc;

    const VERT_SRC: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_nrm_or_col;

uniform mat4 u_mvp;
uniform mat4 u_model;
uniform int u_unlit;

out vec3 v_normal;
out vec3 v_col;

void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_normal = mat3(u_model) * a_nrm_or_col;
    v_col = a_nrm_or_col;
}
"#;

    const FRAG_SRC: &str = r#"#version 330 core
in vec3 v_normal;
in vec3 v_col;

uniform vec3 u_color;
uniform int u_unlit;
uniform float u_selected;

out vec4 FragColor;

void main() {
    if (u_unlit == 1) {
        FragColor = vec4(v_col, 1.0);
        return;
    }
    vec3 n = normalize(v_normal);
    vec3 light_dir = normalize(vec3(0.5, 0.85, 0.45));
    float diff = max(dot(n, light_dir), 0.0);
    vec3 col = u_color * (0.35 + 0.65 * diff);
    col = mix(col, vec3(1.0, 0.55, 0.1), u_selected * 0.18);
    FragColor = vec4(col, 1.0);
}
"#;

    pub struct GpuMesh {
        pub vao: glow::VertexArray,
        pub vbo: glow::Buffer,
        pub vert_count: i32,
    }

    pub struct GlViewportRenderer {
        gl: Arc<glow::Context>,
        program: glow::Program,
        meshes: HashMap<usize, GpuMesh>,
        grid_vao: glow::VertexArray,
        grid_vbo: glow::Buffer,
        grid_vert_count: i32,
        next_mesh_id: usize,
    }

    impl GlViewportRenderer {
        pub fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
            unsafe {
                let program = compile_program(&gl, VERT_SRC, FRAG_SRC)?;
                let cube = create_cube(&gl)?;
                let (grid_vao, grid_vbo, grid_vert_count) = create_grid(&gl, 12)?;
                let mut meshes = HashMap::new();
                meshes.insert(0, cube);
                Ok(Self {
                    gl,
                    program,
                    meshes,
                    grid_vao,
                    grid_vbo,
                    grid_vert_count,
                    next_mesh_id: 1,
                })
            }
        }

        pub fn upload_pos_normal_mesh(&mut self, data: &[f32]) -> Result<usize, String> {
            unsafe {
                let (vao, vbo) = upload_vec6(&self.gl, data)?;
                let id = self.next_mesh_id;
                self.next_mesh_id += 1;
                self.meshes.insert(
                    id,
                    GpuMesh {
                        vao,
                        vbo,
                        vert_count: (data.len() / 6) as i32,
                    },
                );
                Ok(id)
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
                let model_loc = gl.get_uniform_location(self.program, "u_model");
                let unlit_loc = gl.get_uniform_location(self.program, "u_unlit");
                let color_loc = gl.get_uniform_location(self.program, "u_color");
                let sel_loc = gl.get_uniform_location(self.program, "u_selected");

                gl.uniform_1_i32(unlit_loc.as_ref(), 1);
                gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &vp.to_cols_array());
                gl.uniform_matrix_4_f32_slice(
                    model_loc.as_ref(),
                    false,
                    &Mat4::IDENTITY.to_cols_array(),
                );
                gl.bind_vertex_array(Some(self.grid_vao));
                gl.draw_arrays(glow::LINES, 0, self.grid_vert_count);

                gl.uniform_1_i32(unlit_loc.as_ref(), 0);
                for obj in &scene.objects {
                    if !obj.visible {
                        continue;
                    }
                    let Some(mesh) = self.meshes.get(&obj.mesh_id) else {
                        continue;
                    };
                    let world = scene.world_matrix(obj.id)
                        * Mat4::from_translation(Vec3::from_array(obj.pivot_offset));
                    let mvp = vp * world;

                    gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mvp.to_cols_array());
                    gl.uniform_matrix_4_f32_slice(
                        model_loc.as_ref(),
                        false,
                        &world.to_cols_array(),
                    );
                    gl.uniform_3_f32(color_loc.as_ref(), obj.color[0], obj.color[1], obj.color[2]);
                    let sel = if scene.selected_id == Some(obj.id) { 1.0 } else { 0.0 };
                    gl.uniform_1_f32(sel_loc.as_ref(), sel);

                    gl.bind_vertex_array(Some(mesh.vao));
                    gl.draw_arrays(glow::TRIANGLES, 0, mesh.vert_count);
                }

                gl.bind_vertex_array(None);
                gl.use_program(None);
                gl.disable(glow::DEPTH_TEST);
            }
        }

        pub fn destroy(&self) {
            unsafe {
                self.gl.delete_program(self.program);
                for m in self.meshes.values() {
                    self.gl.delete_vertex_array(m.vao);
                    self.gl.delete_buffer(m.vbo);
                }
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

    unsafe fn upload_vec6(
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

    unsafe fn create_cube(gl: &glow::Context) -> Result<GpuMesh, String> {
        #[rustfmt::skip]
        let v: [f32; 216] = [
            -0.5,-0.5, 0.5,  0.0, 0.0, 1.0,   0.5,-0.5, 0.5,  0.0, 0.0, 1.0,   0.5, 0.5, 0.5,  0.0, 0.0, 1.0,
            -0.5,-0.5, 0.5,  0.0, 0.0, 1.0,   0.5, 0.5, 0.5,  0.0, 0.0, 1.0,  -0.5, 0.5, 0.5,  0.0, 0.0, 1.0,
            -0.5,-0.5,-0.5,  0.0, 0.0,-1.0,  -0.5, 0.5,-0.5,  0.0, 0.0,-1.0,   0.5, 0.5,-0.5,  0.0, 0.0,-1.0,
            -0.5,-0.5,-0.5,  0.0, 0.0,-1.0,   0.5, 0.5,-0.5,  0.0, 0.0,-1.0,   0.5,-0.5,-0.5,  0.0, 0.0,-1.0,
            -0.5, 0.5,-0.5,  0.0, 1.0, 0.0,  -0.5, 0.5, 0.5,  0.0, 1.0, 0.0,   0.5, 0.5, 0.5,  0.0, 1.0, 0.0,
            -0.5, 0.5,-0.5,  0.0, 1.0, 0.0,   0.5, 0.5, 0.5,  0.0, 1.0, 0.0,   0.5, 0.5,-0.5,  0.0, 1.0, 0.0,
            -0.5,-0.5,-0.5,  0.0,-1.0, 0.0,   0.5,-0.5,-0.5,  0.0,-1.0, 0.0,   0.5,-0.5, 0.5,  0.0,-1.0, 0.0,
            -0.5,-0.5,-0.5,  0.0,-1.0, 0.0,   0.5,-0.5, 0.5,  0.0,-1.0, 0.0,  -0.5,-0.5, 0.5,  0.0,-1.0, 0.0,
             0.5,-0.5,-0.5,  1.0, 0.0, 0.0,   0.5, 0.5,-0.5,  1.0, 0.0, 0.0,   0.5, 0.5, 0.5,  1.0, 0.0, 0.0,
             0.5,-0.5,-0.5,  1.0, 0.0, 0.0,   0.5, 0.5, 0.5,  1.0, 0.0, 0.0,   0.5,-0.5, 0.5,  1.0, 0.0, 0.0,
            -0.5,-0.5,-0.5, -1.0, 0.0, 0.0,  -0.5,-0.5, 0.5, -1.0, 0.0, 0.0,  -0.5, 0.5, 0.5, -1.0, 0.0, 0.0,
            -0.5,-0.5,-0.5, -1.0, 0.0, 0.0,  -0.5, 0.5, 0.5, -1.0, 0.0, 0.0,  -0.5, 0.5,-0.5, -1.0, 0.0, 0.0,
        ];
        let (vao, vbo) = upload_vec6(gl, &v)?;
        Ok(GpuMesh {
            vao,
            vbo,
            vert_count: 36,
        })
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
        let (vao, vbo) = upload_vec6(gl, &v)?;
        Ok((vao, vbo, count))
    }
}

use core::{InteractionMode, PropertiesTab, Scene, SceneObject, Transform, UnifiedAnimFile, WorkspaceTab};
use glam::Vec3;
use render_api::ViewportCamera;
use render_gl::GlViewportRenderer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModalTool {
    None,
    Grab,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AxisLock {
    None,
    X,
    Y,
    Z,
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("APPCRAFT 0.2"),
        renderer: eframe::Renderer::Glow,
        depth_buffer: 24,
        follow_system_theme: false,
        default_theme: eframe::Theme::Dark,
        ..Default::default()
    };

    eframe::run_native(
        "APPCRAFT",
        options,
        Box::new(|cc| Ok(Box::new(AppCraftApp::new(cc)))),
    )
}

struct AppCraftApp {
    scene: Arc<Mutex<Scene>>,
    camera: ViewportCamera,
    workspace: WorkspaceTab,
    mode: InteractionMode,
    prop_tab: PropertiesTab,
    renderer: Arc<Mutex<Option<GlViewportRenderer>>>,
    status_msg: String,
    modal_tool: ModalTool,
    axis_lock: AxisLock,
    saved_transform: Option<Transform>,
}

impl AppCraftApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_blender_theme(&cc.egui_ctx);
        let renderer = cc
            .gl
            .as_ref()
            .and_then(|gl| GlViewportRenderer::new(Arc::clone(gl)).ok());
        Self {
            scene: Arc::new(Mutex::new(Scene::default())),
            camera: ViewportCamera::default(),
            workspace: WorkspaceTab::Layout,
            mode: InteractionMode::Object,
            prop_tab: PropertiesTab::Object,
            renderer: Arc::new(Mutex::new(renderer)),
            status_msg: "Ready".to_string(),
            modal_tool: ModalTool::None,
            axis_lock: AxisLock::None,
            saved_transform: None,
        }
    }

    fn import_model_file(&mut self, path: &Path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let data_res = if ext == "obj" {
            load_obj_verts(path)
        } else if ext == "glb" || ext == "gltf" {
            load_glb_verts(path)
        } else {
            Err("Unsupported file format".to_string())
        };

        match data_res {
            Ok(verts) if !verts.is_empty() => {
                let mesh_res = self
                    .renderer
                    .lock()
                    .ok()
                    .and_then(|mut g| g.as_mut().map(|r| r.upload_pos_normal_mesh(&verts)));
                if let Some(Ok(mesh_id)) = mesh_res {
                    if let Ok(mut sc) = self.scene.lock() {
                        let id = sc.alloc_id();
                        let fname = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Imported")
                            .to_string();
                        sc.objects.push(SceneObject {
                            id,
                            parent_id: None,
                            name: fname,
                            mesh_id,
                            color: [0.78, 0.78, 0.80],
                            transform: Transform::default(),
                            pivot_offset: [0.0, 0.0, 0.0],
                            keyframes: Vec::new(),
                            visible: true,
                        });
                        sc.selected_id = Some(id);
                    }
                    self.status_msg = format!("Imported: {}", path.display());
                }
            }
            Ok(_) => self.status_msg = "Empty mesh".to_string(),
            Err(e) => self.status_msg = format!("Import error: {}", e),
        }
    }
}

impl eframe::App for AppCraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_blender_theme(ctx);

        let wants_kb = ctx.wants_keyboard_input();
        if !wants_kb {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    if let Ok(mut sc) = self.scene.lock() {
                        sc.timeline.playing = !sc.timeline.playing;
                    }
                }
                if i.key_pressed(egui::Key::I) {
                    if let Ok(mut sc) = self.scene.lock() {
                        let f = sc.timeline.current_frame;
                        if let Some(id) = sc.selected_id {
                            if let Some(obj) = sc.objects.iter_mut().find(|o| o.id == id) {
                                obj.insert_keyframe(f);
                                self.status_msg = format!("Inserted Keyframe at {}", f);
                            }
                        }
                    }
                }
                if i.key_pressed(egui::Key::G) {
                    self.begin_modal(ModalTool::Grab);
                } else if i.key_pressed(egui::Key::R) {
                    self.begin_modal(ModalTool::Rotate);
                } else if i.key_pressed(egui::Key::S) {
                    self.begin_modal(ModalTool::Scale);
                }
                if self.modal_tool != ModalTool::None {
                    if i.key_pressed(egui::Key::X) {
                        self.axis_lock = AxisLock::X;
                    } else if i.key_pressed(egui::Key::Y) {
                        self.axis_lock = AxisLock::Y;
                    } else if i.key_pressed(egui::Key::Z) {
                        self.axis_lock = AxisLock::Z;
                    } else if i.key_pressed(egui::Key::Escape) {
                        self.cancel_modal();
                    } else if i.key_pressed(egui::Key::Enter) {
                        self.modal_tool = ModalTool::None;
                        self.axis_lock = AxisLock::None;
                        self.saved_transform = None;
                    }
                }
            });
        }

        if let Ok(mut sc) = self.scene.lock() {
            if sc.timeline.playing {
                sc.timeline.current_frame += 1;
                if sc.timeline.current_frame > sc.timeline.end_frame {
                    sc.timeline.current_frame = sc.timeline.start_frame;
                }
                sc.evaluate_frame();
                ctx.request_repaint();
            }
        }

        egui::TopBottomPanel::top("top_bar")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label("APPCRAFT 0.2");
                    ui.separator();
                    ui.menu_button("File", |ui| {
                        if ui.button("New Scene").clicked() {
                            if let Ok(mut sc) = self.scene.lock() {
                                *sc = Scene::default();
                            }
                            ui.close_menu();
                        }
                        if ui.button("Import (.glb / .obj)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("3D Models", &["glb", "gltf", "obj"])
                                .pick_file()
                            {
                                self.import_model_file(&p);
                            }
                        }
                        ui.separator();
                        if ui.button("Export Anim (.acanim)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("APPCRAFT Animation", &["acanim", "json"])
                                .set_file_name("animation.acanim")
                                .save_file()
                            {
                                if let Ok(sc) = self.scene.lock() {
                                    if let Ok(txt) = serde_json::to_string_pretty(&sc.export_unified_anim()) {
                                        let _ = std::fs::write(&p, txt);
                                        self.status_msg = format!("Exported: {}", p.display());
                                    }
                                }
                            }
                        }
                        if ui.button("Import Anim (.acanim)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("APPCRAFT Animation", &["acanim", "json"])
                                .pick_file()
                            {
                                if let Ok(txt) = std::fs::read_to_string(&p) {
                                    if let Ok(anim) = serde_json::from_str::<UnifiedAnimFile>(&txt) {
                                        if let Ok(mut sc) = self.scene.lock() {
                                            sc.import_unified_anim(anim);
                                            self.status_msg = "Imported Animation".to_string();
                                        }
                                    }
                                }
                            }
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("Add", |ui| {
                        if ui.button("Steve Rig").clicked() {
                            if let Ok(mut sc) = self.scene.lock() {
                                sc.spawn_steve_rig();
                            }
                            ui.close_menu();
                        }
                        if ui.button("Cube").clicked() {
                            if let Ok(mut sc) = self.scene.lock() {
                                let id = sc.alloc_id();
                                sc.objects.push(SceneObject {
                                    id,
                                    parent_id: None,
                                    name: format!("Cube.{:03}", id),
                                    mesh_id: 0,
                                    color: [0.72, 0.72, 0.75],
                                    transform: Transform::default(),
                                    pivot_offset: [0.0, 0.0, 0.0],
                                    keyframes: Vec::new(),
                                    visible: true,
                                });
                                sc.selected_id = Some(id);
                            }
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
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(&self.status_msg);
                    });
                });
            });

        egui::TopBottomPanel::bottom("timeline")
            .default_height(115.0)
            .resizable(true)
            .show(ctx, |ui| {
                if let Ok(mut sc) = self.scene.lock() {
                    let mut frame_changed = false;
                    ui.horizontal(|ui| {
                        let play_txt = if sc.timeline.playing { "Pause" } else { "Play" };
                        if ui.button(play_txt).clicked() {
                            sc.timeline.playing = !sc.timeline.playing;
                        }
                        if ui.button("Insert Keyframe (I)").clicked() {
                            let f = sc.timeline.current_frame;
                            if let Some(id) = sc.selected_id {
                                if let Some(obj) = sc.objects.iter_mut().find(|o| o.id == id) {
                                    obj.insert_keyframe(f);
                                    self.status_msg = format!("Keyframe saved at {}", f);
                                }
                            }
                        }
                        ui.separator();
                        ui.label("Frame:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut sc.timeline.current_frame)
                                    .range(sc.timeline.start_frame..=sc.timeline.end_frame),
                            )
                            .changed()
                        {
                            frame_changed = true;
                        }
                        ui.label("End:");
                        ui.add(egui::DragValue::new(&mut sc.timeline.end_frame).range(1..=10000));
                    });
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut sc.timeline.current_frame,
                                sc.timeline.start_frame..=sc.timeline.end_frame,
                            )
                            .show_value(false),
                        )
                        .changed()
                    {
                        frame_changed = true;
                    }
                    if frame_changed {
                        sc.evaluate_frame();
                    }
                }
            });

        egui::SidePanel::right("right_dock")
            .default_width(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                if let Ok(mut sc) = self.scene.lock() {
                    let selected_id = sc.selected_id;
                    ui.heading("Outliner");
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for obj in &mut sc.objects {
                                let selected = selected_id == Some(obj.id);
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut obj.visible, "");
                                    let prefix = if obj.parent_id.is_some() { "  |- " } else { "" };
                                    if ui
                                        .selectable_label(selected, format!("{}{}", prefix, obj.name))
                                        .clicked()
                                    {
                                        sc.selected_id = Some(obj.id);
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
                            if let Some(id) = sc.selected_id {
                                if let Some(obj) = sc.objects.iter_mut().find(|o| o.id == id) {
                                    ui.label(format!("{} (Keys: {})", obj.name, obj.keyframes.len()));
                                    draw_vec3(ui, "Location", &mut obj.transform.position, 0.05);
                                    draw_vec3(ui, "Rotation", &mut obj.transform.rotation_deg, 1.0);
                                    draw_vec3(ui, "Scale", &mut obj.transform.scale, 0.05);
                                    ui.color_edit_button_rgb(&mut obj.color);
                                }
                            }
                        }
                        PropertiesTab::Render => {
                            ui.label("Engine: APPCRAFT GL");
                            ui.label("Mode: OpenGL 3.3 Core");
                        }
                        PropertiesTab::Material => {
                            ui.label("Standard PBR Contract");
                        }
                        PropertiesTab::Bone => {
                            ui.label("Parent-Child Rigging Active");
                        }
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
                    ui.label("Orbit: Middle/Right Drag | G/R/S + X/Y/Z | I: Keyframe | Space: Play");
                });

                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(57, 57, 57));

                let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                if self.modal_tool != ModalTool::None {
                    let delta = ctx.input(|i| i.pointer.delta());
                    self.apply_modal_delta(delta);
                    if response.clicked() {
                        self.modal_tool = ModalTool::None;
                        self.axis_lock = AxisLock::None;
                        self.saved_transform = None;
                    } else if response.secondary_clicked() {
                        self.cancel_modal();
                    }
                    ctx.request_repaint();
                } else {
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
                            self.camera.distance =
                                (self.camera.distance - scroll * 0.01).clamp(1.5, 100.0);
                        }
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
                let scene_arc = Arc::clone(&self.scene);
                let camera_clone = self.camera.clone();

                let cb = egui::PaintCallback {
                    rect,
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                        move |_info, _painter| {
                            if let (Ok(r_guard), Ok(s_guard)) =
                                (renderer_arc.lock(), scene_arc.lock())
                            {
                                if let Some(r) = r_guard.as_ref() {
                                    r.render(&s_guard, &camera_clone, vp_rect);
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

impl AppCraftApp {
    fn begin_modal(&mut self, tool: ModalTool) {
        if let Ok(sc) = self.scene.lock() {
            if let Some(id) = sc.selected_id {
                if let Some(obj) = sc.objects.iter().find(|o| o.id == id) {
                    self.saved_transform = Some(obj.transform.clone());
                    self.modal_tool = tool;
                    self.axis_lock = AxisLock::None;
                }
            }
        }
    }

    fn cancel_modal(&mut self) {
        if let Some(saved) = self.saved_transform.take() {
            if let Ok(mut sc) = self.scene.lock() {
                if let Some(id) = sc.selected_id {
                    if let Some(obj) = sc.objects.iter_mut().find(|o| o.id == id) {
                        obj.transform = saved;
                    }
                }
            }
        }
        self.modal_tool = ModalTool::None;
        self.axis_lock = AxisLock::None;
    }

    fn apply_modal_delta(&mut self, delta: egui::Vec2) {
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }
        if let Ok(mut sc) = self.scene.lock() {
            if let Some(id) = sc.selected_id {
                if let Some(obj) = sc.objects.iter_mut().find(|o| o.id == id) {
                    match self.modal_tool {
                        ModalTool::Grab => {
                            let s = 0.015;
                            match self.axis_lock {
                                AxisLock::X => obj.transform.position[0] += delta.x * s,
                                AxisLock::Y => obj.transform.position[1] -= delta.y * s,
                                AxisLock::Z => obj.transform.position[2] += delta.y * s,
                                AxisLock::None => {
                                    obj.transform.position[0] += delta.x * s;
                                    obj.transform.position[1] -= delta.y * s;
                                }
                            }
                        }
                        ModalTool::Rotate => {
                            let s = 0.5;
                            match self.axis_lock {
                                AxisLock::X => obj.transform.rotation_deg[0] += delta.x * s,
                                AxisLock::Y => obj.transform.rotation_deg[1] += delta.x * s,
                                AxisLock::Z => obj.transform.rotation_deg[2] += delta.x * s,
                                AxisLock::None => obj.transform.rotation_deg[1] += delta.x * s,
                            }
                        }
                        ModalTool::Scale => {
                            let s = 0.01 * delta.x;
                            match self.axis_lock {
                                AxisLock::X => obj.transform.scale[0] = (obj.transform.scale[0] + s).max(0.05),
                                AxisLock::Y => obj.transform.scale[1] = (obj.transform.scale[1] + s).max(0.05),
                                AxisLock::Z => obj.transform.scale[2] = (obj.transform.scale[2] + s).max(0.05),
                                AxisLock::None => {
                                    for k in 0..3 {
                                        obj.transform.scale[k] = (obj.transform.scale[k] + s).max(0.05);
                                    }
                                }
                            }
                        }
                        ModalTool::None => {}
                    }
                }
            }
        }
    }
}

fn load_obj_verts(path: &Path) -> Result<Vec<f32>, String> {
    let (models, _) = tobj::load_obj(path, &tobj::GPU_LOAD_OPTIONS).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for m in models {
        let pos = &m.mesh.positions;
        let nrm = &m.mesh.normals;
        let idx = &m.mesh.indices;
        for tri in idx.chunks_exact(3) {
            let p0 = Vec3::new(pos[tri[0] as usize * 3], pos[tri[0] as usize * 3 + 1], pos[tri[0] as usize * 3 + 2]);
            let p1 = Vec3::new(pos[tri[1] as usize * 3], pos[tri[1] as usize * 3 + 1], pos[tri[1] as usize * 3 + 2]);
            let p2 = Vec3::new(pos[tri[2] as usize * 3], pos[tri[2] as usize * 3 + 1], pos[tri[2] as usize * 3 + 2]);
            let fallback_n = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            for &vi in tri {
                let i = vi as usize * 3;
                out.extend_from_slice(&[pos[i], pos[i + 1], pos[i + 2]]);
                if nrm.len() >= i + 3 {
                    out.extend_from_slice(&[nrm[i], nrm[i + 1], nrm[i + 2]]);
                } else {
                    out.extend_from_slice(&[fallback_n.x, fallback_n.y, fallback_n.z]);
                }
            }
        }
    }
    Ok(out)
}

fn load_glb_verts(path: &Path) -> Result<Vec<f32>, String> {
    let (doc, buffers, _) = gltf::import(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(positions) = reader.read_positions() else { continue };
            let pos_vec: Vec<[f32; 3]> = positions.collect();
            let nrm_vec: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_default();

            let indices: Vec<u32> = if let Some(idx) = reader.read_indices() {
                idx.into_u32().collect()
            } else {
                (0..pos_vec.len() as u32).collect()
            };

            for tri in indices.chunks_exact(3) {
                let p0 = Vec3::from_array(pos_vec[tri[0] as usize]);
                let p1 = Vec3::from_array(pos_vec[tri[1] as usize]);
                let p2 = Vec3::from_array(pos_vec[tri[2] as usize]);
                let fnrm = (p1 - p0).cross(p2 - p0).normalize_or_zero();
                for &vi in tri {
                    let p = pos_vec[vi as usize];
                    let n = nrm_vec.get(vi as usize).copied().unwrap_or([fnrm.x, fnrm.y, fnrm.z]);
                    out.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2]]);
                }
            }
        }
    }
    Ok(out)
}

fn draw_vec3(ui: &mut egui::Ui, label: &str, v: &mut [f32; 3], speed: f32) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut v[0]).speed(speed).prefix("X: "));
        ui.add(egui::DragValue::new(&mut v[1]).speed(speed).prefix("Y: "));
        ui.add(egui::DragValue::new(&mut v[2]).speed(speed).prefix("Z: "));
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
