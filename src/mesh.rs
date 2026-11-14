use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SelectMode {
    Vertex,
    Edge,
    Face,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub selected: bool,
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub v0: usize,
    pub v1: usize,
    pub selected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Face {
    pub indices: Vec<usize>,
    pub normal: Vec3,
    pub selected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditableMesh {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
}

impl EditableMesh {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            faces: Vec::new(),
        }
    }

    pub fn new_cube(size: f32) -> Self {
        let h = size * 0.5;
        let p = [
            Vec3::new(-h, -h,  h), Vec3::new( h, -h,  h),
            Vec3::new( h,  h,  h), Vec3::new(-h,  h,  h),
            Vec3::new(-h, -h, -h), Vec3::new( h, -h, -h),
            Vec3::new( h,  h, -h), Vec3::new(-h,  h, -h),
        ];

        let mut vertices: Vec<Vertex> = p.iter().map(|&pos| Vertex {
            position: pos,
            normal: pos.normalize_or_zero(),
            uv: Vec2::ZERO,
            selected: false,
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }).collect();

        let face_indices = [
            [0, 1, 2, 3], // Front
            [5, 4, 7, 6], // Back
            [4, 0, 3, 7], // Left
            [1, 5, 6, 2], // Right
            [3, 2, 6, 7], // Top
            [4, 5, 1, 0], // Bottom
        ];

        let mut faces = Vec::new();
        let mut edges = Vec::new();

        for f in face_indices {
            let p0 = vertices[f[0]].position;
            let p1 = vertices[f[1]].position;
            let p2 = vertices[f[2]].position;
            let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();

            faces.push(Face {
                indices: f.to_vec(),
                normal,
                selected: false,
            });

            for i in 0..4 {
                let v0 = f[i];
                let v1 = f[(i + 1) % 4];
                let min_idx = v0.min(v1);
                let max_idx = v0.max(v1);
                if !edges.iter().any(|e: &Edge| e.v0 == min_idx && e.v1 == max_idx) {
                    edges.push(Edge { v0: min_idx, v1: max_idx, selected: false });
                }
            }
        }

        Self { vertices, edges, faces }
    }

    pub fn select_all(&mut self, select: bool) {
        for v in &mut self.vertices { v.selected = select; }
        for e in &mut self.edges { e.selected = select; }
        for f in &mut self.faces { f.selected = select; }
    }

    pub fn sync_selection_from_verts(&mut self) {
        for e in &mut self.edges {
            e.selected = self.vertices[e.v0].selected && self.vertices[e.v1].selected;
        }
        for f in &mut self.faces {
            f.selected = f.indices.iter().all(|&idx| self.vertices[idx].selected);
        }
    }

    pub fn extrude_selected_faces(&mut self) -> bool {
        let selected_face_indices: Vec<usize> = self.faces.iter().enumerate()
            .filter_map(|(i, f)| if f.selected { Some(i) } else { None })
            .collect();

        if selected_face_indices.is_empty() {
            return false;
        }

        let mut old_to_new_verts = std::collections::HashMap::new();

        for &fi in &selected_face_indices {
            let f = &self.faces[fi];
            for &vi in &f.indices {
                if !old_to_new_verts.contains_key(&vi) {
                    let mut new_v = self.vertices[vi].clone();
                    new_v.selected = true;
                    self.vertices[vi].selected = false;
                    let new_idx = self.vertices.len();
                    self.vertices.push(new_v);
                    old_to_new_verts.insert(vi, new_idx);
                }
            }
        }

        for &fi in &selected_face_indices {
            let old_indices = self.faces[fi].indices.clone();
            let count = old_indices.len();
            for i in 0..count {
                let v_curr = old_indices[i];
                let v_next = old_indices[(i + 1) % count];
                let new_curr = old_to_new_verts[&v_curr];
                let new_next = old_to_new_verts[&v_next];

                let p0 = self.vertices[v_curr].position;
                let p1 = self.vertices[v_next].position;
                let p2 = self.vertices[new_next].position;
                let side_normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();

                self.faces.push(Face {
                    indices: vec![v_curr, v_next, new_next, new_curr],
                    normal: side_normal,
                    selected: false,
                });

                self.edges.push(Edge { v0: v_curr.min(new_curr), v1: v_curr.max(new_curr), selected: false });
                self.edges.push(Edge { v0: new_curr.min(new_next), v1: new_curr.max(new_next), selected: true });
            }

            let new_face_indices: Vec<usize> = old_indices.iter().map(|vi| old_to_new_verts[vi]).collect();
            self.faces[fi].indices = new_face_indices;
            self.faces[fi].selected = true;
        }

        self.recompute_normals();
        true
    }

    pub fn subdivide(&mut self) {
        let mut new_mesh = Self::empty();
        for f in &self.faces {
            if f.indices.len() == 4 {
                let v0 = self.vertices[f.indices[0]].position;
                let v1 = self.vertices[f.indices[1]].position;
                let v2 = self.vertices[f.indices[2]].position;
                let v3 = self.vertices[f.indices[3]].position;

                let c = (v0 + v1 + v2 + v3) * 0.25;
                let m01 = (v0 + v1) * 0.5;
                let m12 = (v1 + v2) * 0.5;
                let m23 = (v2 + v3) * 0.5;
                let m30 = (v3 + v0) * 0.5;

                let start = new_mesh.vertices.len();
                let positions = [v0, m01, c, m30, v1, m12, v2, m23, v3];
                for &pos in &positions {
                    new_mesh.vertices.push(Vertex {
                        position: pos,
                        normal: f.normal,
                        uv: Vec2::ZERO,
                        selected: false,
                        bone_indices: self.vertices[f.indices[0]].bone_indices,
                        bone_weights: self.vertices[f.indices[0]].bone_weights,
                    });
                }

                let sub_quads = [
                    [start, start+1, start+2, start+3],
                    [start+1, start+4, start+5, start+2],
                    [start+2, start+5, start+6, start+7],
                    [start+3, start+2, start+7, start+8],
                ];

                for sq in sub_quads {
                    new_mesh.faces.push(Face {
                        indices: sq.to_vec(),
                        normal: f.normal,
                        selected: false,
                    });
                }
            }
        }
        if !new_mesh.faces.is_empty() {
            new_mesh.rebuild_edges();
            new_mesh.recompute_normals();
            *self = new_mesh;
        }
    }

    pub fn rebuild_edges(&mut self) {
        self.edges.clear();
        for f in &self.faces {
            let count = f.indices.len();
            for i in 0..count {
                let v0 = f.indices[i].min(f.indices[(i + 1) % count]);
                let v1 = f.indices[i].max(f.indices[(i + 1) % count]);
                if !self.edges.iter().any(|e| e.v0 == v0 && e.v1 == v1) {
                    self.edges.push(Edge { v0, v1, selected: false });
                }
            }
        }
    }

    pub fn recompute_normals(&mut self) {
        for f in &mut self.faces {
            if f.indices.len() >= 3 {
                let p0 = self.vertices[f.indices[0]].position;
                let p1 = self.vertices[f.indices[1]].position;
                let p2 = self.vertices[f.indices[2]].position;
                f.normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            }
        }
        for v in &mut self.vertices {
            v.normal = Vec3::ZERO;
        }
        for f in &self.faces {
            for &idx in &f.indices {
                self.vertices[idx].normal += f.normal;
            }
        }
        for v in &mut self.vertices {
            v.normal = v.normal.normalize_or_zero();
        }
    }

    pub fn to_render_triangles(&self) -> Vec<f32> {
        let mut buffer = Vec::new();
        for f in &self.faces {
            if f.indices.len() >= 3 {
                for i in 1..f.indices.len() - 1 {
                    let tri = [f.indices[0], f.indices[i], f.indices[i + 1]];
                    for &idx in &tri {
                        let v = &self.vertices[idx];
                        buffer.extend_from_slice(&[
                            v.position.x, v.position.y, v.position.z,
                            v.normal.x, v.normal.y, v.normal.z,
                        ]);
                    }
                }
            }
        }
        buffer
    }

    pub fn raycast_closest_vertex(&self, ray_origin: Vec3, ray_dir: Vec3, threshold: f32) -> Option<usize> {
        let mut closest = None;
        let mut min_dist = threshold;

        for (i, v) in self.vertices.iter().enumerate() {
            let to_v = v.position - ray_origin;
            let projection = to_v.dot(ray_dir);
            if projection > 0.0 {
                let perp_dist = (to_v - ray_dir * projection).length();
                if perp_dist < min_dist {
                    min_dist = perp_dist;
                    closest = Some(i);
                }
            }
        }
        closest
    }
      }
