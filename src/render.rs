use crate::armature::Armature;
use crate::mesh::{EditableMesh, SelectMode};
use egui_glow::glow::{self, HasContext};
use glam::{Mat4, Vec3};
use std::sync::Arc;

const VERT_SRC: &str = r#"#version 330 core
layout (location = 0) in vec3 a_pos;
layout (location = 1) in vec3 a_norm_or_col;

uniform mat4 u_mvp;
uniform mat4 u_model;
uniform int u_mode; // 0 = shaded mesh, 1 = unlit lines, 2 = points

out vec3 v_normal;
out vec3 v_data;

void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_normal = mat3(u_model) * a_norm_or_col;
    v_data = a_norm_or_col;
    if (u_mode == 2) {
        gl_PointSize = 7.0;
    }
}
"#;

const FRAG_SRC: &str = r#"#version 330 core
in vec3 v_normal;
in vec3 v_data;

uniform int u_mode;
uniform vec3 u_base_color;
uniform float u_selected;

out vec4 FragColor;

void main() {
    if (u_mode == 1) { // Unlit Lines / Grid / Bone X-Ray
        FragColor = vec4(v_data, 1.0);
        return;
    }
    if (u_mode == 2) { // Vertices in Edit Mode
        FragColor = vec4(v_data, 1.0);
        return;
    }

    vec3 n = normalize(v_normal);
    vec3 sun = normalize(vec3(0.55, 0.85, 0.45));
    float diff = max(dot(n, sun), 0.0);
    vec3 col = u_base_color * (0.35 + 0.65 * diff);

    if (u_selected > 0.5) {
        col = mix(col, vec3(1.0, 0.55, 0.0), 0.25);
    }

    FragColor = vec4(col, 1.0);
}
"#;

pub struct GlUnifiedRenderer {
    gl: Arc<glow::Context>,
    program: glow::Program,
    dyn_vao: glow::VertexArray,
    dyn_vbo: glow::Buffer,
    grid_vao: glow::VertexArray,
    grid_vbo: glow::Buffer,
    grid_vert_count: i32,
}

impl GlUnifiedRenderer {
    pub fn new(gl: Arc<glow::Context>) -> Result<Self, String> {
        unsafe {
            let program = compile_program(&gl, VERT_SRC, FRAG_SRC)?;
            let dyn_vao = gl.create_vertex_array()?;
            let dyn_vbo = gl.create_buffer()?;

            let (grid_vao, grid_vbo, grid_vert_count) = create_grid(&gl, 16)?;

            Ok(Self {
                gl,
                program,
                dyn_vao,
                dyn_vbo,
                grid_vao,
                grid_vbo,
                grid_vert_count,
            })
        }
    }

    pub fn begin_frame(&self, vp_rect: [i32; 4]) {
        let [x, y, w, h] = vp_rect;
        if w <= 0 || h <= 0 { return; }
        unsafe {
            let gl = &self.gl;
            gl.viewport(x, y, w, h);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.use_program(Some(self.program));
        }
    }

    pub fn draw_grid(&self, vp: &Mat4) {
        unsafe {
            let gl = &self.gl;
            self.set_uniforms(vp, &Mat4::IDENTITY, 1, Vec3::ONE, 0.0);
            gl.bind_vertex_array(Some(self.grid_vao));
            gl.draw_arrays(glow::LINES, 0, self.grid_vert_count);
        }
    }

    pub fn draw_editable_mesh(
        &self,
        mesh: &EditableMesh,
        world: &Mat4,
        vp: &Mat4,
        selected: bool,
        is_edit_mode: bool,
        select_mode: SelectMode,
    ) {
        let tris = mesh.to_render_triangles();
        if tris.is_empty() { return; }

        let mvp = *vp * *world;
        unsafe {
            let gl = &self.gl;
            self.upload_dyn_buffer(&tris);

            // 1. Shaded Faces
            self.set_uniforms(&mvp, world, 0, Vec3::new(0.72, 0.72, 0.74), if selected { 1.0 } else { 0.0 });
            gl.draw_arrays(glow::TRIANGLES, 0, (tris.len() / 6) as i32);

            // 2. Wireframe / Outlines / Edit Mode Overlays
            if selected || is_edit_mode {
                let mut edge_verts = Vec::new();
                for e in &mesh.edges {
                    let v0 = mesh.vertices[e.v0].position;
                    let v1 = mesh.vertices[e.v1].position;
                    let col = if is_edit_mode {
                        if e.selected { Vec3::new(1.0, 0.55, 0.0) } else { Vec3::new(0.15, 0.15, 0.15) }
                    } else {
                        Vec3::new(1.0, 0.55, 0.0) // Orange outline in Object Mode
                    };
                    edge_verts.extend_from_slice(&[v0.x, v0.y, v0.z, col.x, col.y, col.z]);
                    edge_verts.extend_from_slice(&[v1.x, v1.y, v1.z, col.x, col.y, col.z]);
                }

                if !edge_verts.is_empty() {
                    self.upload_dyn_buffer(&edge_verts);
                    self.set_uniforms(&mvp, world, 1, Vec3::ONE, 0.0);
                    gl.draw_arrays(glow::LINES, 0, (edge_verts.len() / 6) as i32);
                }
            }

            // 3. Edit Mode Vertices Points
            if is_edit_mode && select_mode == SelectMode::Vertex {
                let mut p_verts = Vec::new();
                for v in &mesh.vertices {
                    let col = if v.selected { Vec3::new(1.0, 0.55, 0.0) } else { Vec3::new(0.05, 0.05, 0.05) };
                    p_verts.extend_from_slice(&[v.position.x, v.position.y, v.position.z, col.x, col.y, col.z]);
                }
                if !p_verts.is_empty() {
                    gl.disable(glow::DEPTH_TEST); // Draw vertices on top
                    self.upload_dyn_buffer(&p_verts);
                    self.set_uniforms(&mvp, world, 2, Vec3::ONE, 0.0);
                    gl.draw_arrays(glow::POINTS, 0, (p_verts.len() / 6) as i32);
                    gl.enable(glow::DEPTH_TEST);
                }
            }
        }
    }

    pub fn draw_armature_xray(&self, arm: &mut Armature, world: &Mat4, vp: &Mat4, _is_pose_mode: bool) {
        let pose_matrices = arm.compute_world_pose_matrices();
        let mut bone_lines = Vec::new();

        for (i, b) in arm.bones.iter().enumerate() {
            let m = *world * pose_matrices[i];
            let head = m.transform_point3(Vec3::ZERO);
            let tail = m.transform_point3(Vec3::new(0.0, b.length(), 0.0));

            let col = if b.selected { Vec3::new(0.2, 0.85, 1.0) } else { Vec3::new(0.9, 0.7, 0.2) };

            bone_lines.extend_from_slice(&[head.x, head.y, head.z, col.x, col.y, col.z]);
            bone_lines.extend_from_slice(&[tail.x, tail.y, tail.z, col.x, col.y, col.z]);
        }

        if !bone_lines.is_empty() {
            unsafe {
                let gl = &self.gl;
                gl.disable(glow::DEPTH_TEST); // X-Ray Mode: visible through geometry
                self.upload_dyn_buffer(&bone_lines);
                self.set_uniforms(vp, &Mat4::IDENTITY, 1, Vec3::ONE, 0.0);
                gl.draw_arrays(glow::LINES, 0, (bone_lines.len() / 6) as i32);
                gl.enable(glow::DEPTH_TEST);
            }
        }
    }

    pub fn end_frame(&self) {
        unsafe {
            self.gl.bind_vertex_array(None);
            self.gl.use_program(None);
            self.gl.disable(glow::DEPTH_TEST);
        }
    }

    unsafe fn upload_dyn_buffer(&self, data: &[f32]) {
        let gl = &self.gl;
        gl.bind_vertex_array(Some(self.dyn_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.dyn_vbo));
        let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::DYNAMIC_DRAW);

        let stride = 6 * std::mem::size_of::<f32>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
    }

    unsafe fn set_uniforms(&self, mvp: &Mat4, model: &Mat4, mode: i32, color: Vec3, selected: f32) {
        let gl = &self.gl;
        let mvp_loc = gl.get_uniform_location(self.program, "u_mvp");
        let model_loc = gl.get_uniform_location(self.program, "u_model");
        let mode_loc = gl.get_uniform_location(self.program, "u_mode");
        let col_loc = gl.get_uniform_location(self.program, "u_base_color");
        let sel_loc = gl.get_uniform_location(self.program, "u_selected");

        gl.uniform_matrix_4_f32_slice(mvp_loc.as_ref(), false, &mvp.to_cols_array());
        gl.uniform_matrix_4_f32_slice(model_loc.as_ref(), false, &model.to_cols_array());
        gl.uniform_1_i32(mode_loc.as_ref(), mode);
        gl.uniform_3_f32(col_loc.as_ref(), color.x, color.y, color.z);
        gl.uniform_1_f32(sel_loc.as_ref(), selected);
    }

    pub fn destroy(&self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.dyn_vao);
            self.gl.delete_buffer(self.dyn_vbo);
            self.gl.delete_vertex_array(self.grid_vao);
            self.gl.delete_buffer(self.grid_vbo);
        }
    }
}

unsafe fn compile_program(gl: &glow::Context, vert: &str, frag: &str) -> Result<glow::Program, String> {
    let program = gl.create_program()?;
    let vs = gl.create_shader(glow::VERTEX_SHADER)?;
    gl.shader_source(vs, vert);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) { return Err(gl.get_shader_info_log(vs)); }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER)?;
    gl.shader_source(fs, frag);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) { return Err(gl.get_shader_info_log(fs)); }

    gl.attach_shader(program, vs);
    gl.attach_shader(program, fs);
    gl.link_program(program);
    if !gl.get_program_link_status(program) { return Err(gl.get_program_info_log(program)); }

    gl.delete_shader(vs);
    gl.delete_shader(fs);
    Ok(program)
}

unsafe fn create_grid(gl: &glow::Context, half: i32) -> Result<(glow::VertexArray, glow::Buffer, i32), String> {
    let mut v = Vec::new();
    let f = half as f32;
    for i in -half..=half {
        let p = i as f32;
        if i == 0 {
            v.extend_from_slice(&[-f, 0.0, 0.0, 0.75, 0.25, 0.25, f, 0.0, 0.0, 0.75, 0.25, 0.25]); // X Red
            v.extend_from_slice(&[0.0, 0.0, -f, 0.25, 0.75, 0.25, 0.0, 0.0, f, 0.25, 0.75, 0.25]); // Y Green
        } else {
            v.extend_from_slice(&[-f, 0.0, p, 0.26, 0.26, 0.26, f, 0.0, p, 0.26, 0.26, 0.26]);
            v.extend_from_slice(&[p, 0.0, -f, 0.26, 0.26, 0.26, p, 0.0, f, 0.26, 0.26, 0.26]);
        }
    }
    let vao = gl.create_vertex_array()?;
    let vbo = gl.create_buffer()?;
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(&v));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);

    let stride = 6 * std::mem::size_of::<f32>() as i32;
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
    gl.bind_vertex_array(None);

    Ok((vao, vbo, (v.len() / 6) as i32))
                      }
