use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub head: Vec3,
    pub tail: Vec3,
    pub roll: f32,
    pub selected: bool,
    pub pose_position: Vec3,
    pub pose_rotation: Vec3,
    pub pose_scale: Vec3,
}

impl Bone {
    pub fn new(name: &str, head: Vec3, tail: Vec3, parent: Option<usize>) -> Self {
        Self {
            name: name.to_string(),
            parent,
            head,
            tail,
            roll: 0.0,
            selected: false,
            pose_position: Vec3::ZERO,
            pose_rotation: Vec3::ZERO,
            pose_scale: Vec3::ONE,
        }
    }

    pub fn length(&self) -> f32 {
        (self.tail - self.head).length()
    }

    pub fn local_rest_matrix(&self) -> Mat4 {
        let dir = (self.tail - self.head).normalize_or_zero();
        let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let right = dir.cross(up).normalize();
        let actual_up = right.cross(dir).normalize();

        Mat4::from_cols(
            right.extend(0.0),
            dir.extend(0.0),
            actual_up.extend(0.0),
            self.head.extend(1.0),
        )
    }

    pub fn pose_matrix(&self) -> Mat4 {
        let r = self.pose_rotation * std::f32::consts::PI / 180.0;
        let q = Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z);
        Mat4::from_scale_rotation_translation(self.pose_scale, q, self.pose_position)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IKConstraint {
    pub tip_bone: usize,
    pub mid_bone: usize,
    pub root_bone: usize,
    pub target: Vec3,
    pub pole_target: Option<Vec3>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Armature {
    pub bones: Vec<Bone>,
    pub ik_constraints: Vec<IKConstraint>,
}

impl Armature {
    pub fn empty() -> Self {
        Self {
            bones: Vec::new>,
            ik_constraints: Vec::new(),
        }
    }

    pub fn new_steve_armature() -> Self {
        let mut arm = Self::empty();

        arm.bones.push(Bone::new("Root", Vec3::new(0.0, 0.75, 0.0), Vec3::new(0.0, 1.125, 0.0), None));
        arm.bones.push(Bone::new("Spine", Vec3::new(0.0, 1.125, 0.0), Vec3::new(0.0, 1.5, 0.0), Some(0)));
        arm.bones.push(Bone::new("Head", Vec3::new(0.0, 1.5, 0.0), Vec3::new(0.0, 2.0, 0.0), Some(1)));

        arm.bones.push(Bone::new("Arm_Upper_R", Vec3::new(0.375, 1.375, 0.0), Vec3::new(0.375, 1.0, 0.0), Some(1)));
        arm.bones.push(Bone::new("Arm_Lower_R", Vec3::new(0.375, 1.0, 0.0), Vec3::new(0.375, 0.625, 0.0), Some(3)));

        arm.bones.push(Bone::new("Arm_Upper_L", Vec3::new(-0.375, 1.375, 0.0), Vec3::new(-0.375, 1.0, 0.0), Some(1)));
        arm.bones.push(Bone::new("Arm_Lower_L", Vec3::new(-0.375, 1.0, 0.0), Vec3::new(-0.375, 0.625, 0.0), Some(5)));

        arm.bones.push(Bone::new("Leg_Upper_R", Vec3::new(0.125, 0.75, 0.0), Vec3::new(0.125, 0.375, 0.0), Some(0)));
        arm.bones.push(Bone::new("Leg_Lower_R", Vec3::new(0.125, 0.375, 0.0), Vec3::new(0.125, 0.0, 0.0), Some(7)));

        arm.bones.push(Bone::new("Leg_Upper_L", Vec3::new(-0.125, 0.75, 0.0), Vec3::new(-0.125, 0.375, 0.0), Some(0)));
        arm.bones.push(Bone::new("Leg_Lower_L", Vec3::new(-0.125, 0.375, 0.0), Vec3::new(-0.125, 0.0, 0.0), Some(9)));

        arm.ik_constraints.push(IKConstraint {
            tip_bone: 8,
            mid_bone: 7,
            root_bone: 0,
            target: Vec3::new(0.125, 0.0, 0.0),
            pole_target: Some(Vec3::new(0.125, 0.375, 1.0)),
            enabled: false,
        });
        arm.ik_constraints.push(IKConstraint {
            tip_bone: 10,
            mid_bone: 9,
            root_bone: 0,
            target: Vec3::new(-0.125, 0.0, 0.0),
            pole_target: Some(Vec3::new(-0.125, 0.375, 1.0)),
            enabled: false,
        });

        arm
    }

    pub fn compute_world_pose_matrices(&self) -> Vec<Mat4> {
        let mut mats = vec![Mat4::IDENTITY; self.bones.len()];
        for (i, b) in self.bones.iter().enumerate() {
            let local_pose = b.local_rest_matrix() * b.pose_matrix();
            mats[i] = if let Some(parent_idx) = b.parent {
                mats[parent_idx] * local_pose
            } else {
                local_pose
            };
        }
        mats
    }

    pub fn solve_ik(&mut self) {
        for ik in &self.ik_constraints {
            if !ik.enabled { continue; }
            let p_root = self.bones[ik.mid_bone].head;
            let len_a = self.bones[ik.mid_bone].length();
            let len_b = self.bones[ik.tip_bone].length();
            let target_vec = ik.target - p_root;
            let dist = target_vec.length().clamp(0.001, len_a + len_b - 0.001);

            let cos_angle_a = ((len_a * len_a + dist * dist - len_b * len_b) / (2.0 * len_a * dist)).clamp(-1.0, 1.0);
            let angle_a = cos_angle_a.acos();

            let cos_angle_mid = ((len_a * len_a + len_b * len_b - dist * dist) / (2.0 * len_a * len_b)).clamp(-1.0, 1.0);
            let angle_mid = std::f32::consts::PI - cos_angle_mid.acos();

            let deg_mid = angle_mid.to_degrees();
            let deg_a = angle_a.to_degrees();

            self.bones[ik.mid_bone].pose_rotation.x = -deg_a;
            self.bones[ik.tip_bone].pose_rotation.x = deg_mid;
        }
    }

    pub fn clear_pose_transforms(&mut self) {
        for b in &mut self.bones {
            b.pose_position = Vec3::ZERO;
            b.pose_rotation = Vec3::ZERO;
            b.pose_scale = Vec3::ONE;
        }
    }

    pub fn calculate_auto_weights(&self, vertices: &mut [crate::mesh::Vertex]) {
        for v in vertices.iter_mut() {
            let mut distances = Vec::new();
            for (bi, b) in self.bones.iter().enumerate() {
                let seg = b.tail - b.head;
                let len_sq = seg.length_squared();
                let t = if len_sq < 0.0001 { 0.0 } else { ((v.position - b.head).dot(seg) / len_sq).clamp(0.0, 1.0) };
                let proj = b.head + seg * t;
                let dist = (v.position - proj).length();
                distances.push((bi, dist));
            }

            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut total_w = 0.0;
            let count = distances.len().min(4);
            for i in 0..count {
                let bi = distances[i].0;
                let d = distances[i].1.max(0.001);
                let w = 1.0 / (d * d);
                v.bone_indices[i] = bi as u32;
                v.bone_weights[i] = w;
                total_w += w;
            }

            if total_w > 0.0 {
                for i in 0..count {
                    v.bone_weights[i] /= total_w;
                }
            } else {
                v.bone_weights = [1.0, 0.0, 0.0, 0.0];
            }
        }
    }
}
