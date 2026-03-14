use bevy::prelude::*;

pub(crate) fn create_merged_box_mesh(boxes: &[(Vec3, Vec3)]) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for (pos, scale) in boxes {
        let half = *scale * 0.5;
        let offset = positions.len() as u32;

        // Standard 6 faces of a cube
        let vertices = [
            // Top
            (
                [pos.x - half.x, pos.y + half.y, pos.z - half.z],
                [0.0, 1.0, 0.0],
                [0.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z - half.z],
                [0.0, 1.0, 0.0],
                [1.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z + half.z],
                [0.0, 1.0, 0.0],
                [1.0, 1.0],
            ),
            (
                [pos.x - half.x, pos.y + half.y, pos.z + half.z],
                [0.0, 1.0, 0.0],
                [0.0, 1.0],
            ),
            // Bottom
            (
                [pos.x - half.x, pos.y - half.y, pos.z - half.z],
                [0.0, -1.0, 0.0],
                [0.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y - half.y, pos.z - half.z],
                [0.0, -1.0, 0.0],
                [1.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y - half.y, pos.z + half.z],
                [0.0, -1.0, 0.0],
                [1.0, 1.0],
            ),
            (
                [pos.x - half.x, pos.y - half.y, pos.z + half.z],
                [0.0, -1.0, 0.0],
                [0.0, 1.0],
            ),
            // Front
            (
                [pos.x - half.x, pos.y - half.y, pos.z + half.z],
                [0.0, 0.0, 1.0],
                [0.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y - half.y, pos.z + half.z],
                [0.0, 0.0, 1.0],
                [1.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z + half.z],
                [0.0, 0.0, 1.0],
                [1.0, 1.0],
            ),
            (
                [pos.x - half.x, pos.y + half.y, pos.z + half.z],
                [0.0, 0.0, 1.0],
                [0.0, 1.0],
            ),
            // Back
            (
                [pos.x - half.x, pos.y - half.y, pos.z - half.z],
                [0.0, 0.0, -1.0],
                [0.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y - half.y, pos.z - half.z],
                [0.0, 0.0, -1.0],
                [1.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z - half.z],
                [0.0, 0.0, -1.0],
                [1.0, 1.0],
            ),
            (
                [pos.x - half.x, pos.y + half.y, pos.z - half.z],
                [0.0, 0.0, -1.0],
                [0.0, 1.0],
            ),
            // Left
            (
                [pos.x - half.x, pos.y - half.y, pos.z - half.z],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0],
            ),
            (
                [pos.x - half.x, pos.y + half.y, pos.z - half.z],
                [-1.0, 0.0, 0.0],
                [1.0, 0.0],
            ),
            (
                [pos.x - half.x, pos.y + half.y, pos.z + half.z],
                [-1.0, 0.0, 0.0],
                [1.0, 1.0],
            ),
            (
                [pos.x - half.x, pos.y - half.y, pos.z + half.z],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0],
            ),
            // Right
            (
                [pos.x + half.x, pos.y - half.y, pos.z - half.z],
                [1.0, 0.0, 0.0],
                [0.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z - half.z],
                [1.0, 0.0, 0.0],
                [1.0, 0.0],
            ),
            (
                [pos.x + half.x, pos.y + half.y, pos.z + half.z],
                [1.0, 0.0, 0.0],
                [1.0, 1.0],
            ),
            (
                [pos.x + half.x, pos.y - half.y, pos.z + half.z],
                [1.0, 0.0, 0.0],
                [0.0, 1.0],
            ),
        ];

        for (p, n, u) in vertices {
            positions.push(p);
            normals.push(n);
            uvs.push(u);
        }

        for face in 0..6 {
            let base = offset + face * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}

pub(crate) fn create_merged_box_mesh_rotated(boxes: &[(Vec3, Quat, Vec3)]) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for (pos, rot, scale) in boxes {
        let half = *scale * 0.5;
        let offset = positions.len() as u32;

        let local_vertices = [
            // Top (Y+)
            ([-half.x, half.y, -half.z], [0.0, 1.0, 0.0], [0.0, 0.0]),
            ([half.x, half.y, -half.z], [0.0, 1.0, 0.0], [1.0, 0.0]),
            ([half.x, half.y, half.z], [0.0, 1.0, 0.0], [1.0, 1.0]),
            ([-half.x, half.y, half.z], [0.0, 1.0, 0.0], [0.0, 1.0]),
            // Bottom (Y-)
            ([-half.x, -half.y, -half.z], [0.0, -1.0, 0.0], [0.0, 0.0]),
            ([half.x, -half.y, -half.z], [0.0, -1.0, 0.0], [1.0, 0.0]),
            ([half.x, -half.y, half.z], [0.0, -1.0, 0.0], [1.0, 1.0]),
            ([-half.x, -half.y, half.z], [0.0, -1.0, 0.0], [0.0, 1.0]),
            // Front (Z+)
            ([-half.x, -half.y, half.z], [0.0, 0.0, 1.0], [0.0, 0.0]),
            ([half.x, -half.y, half.z], [0.0, 0.0, 1.0], [1.0, 0.0]),
            ([half.x, half.y, half.z], [0.0, 0.0, 1.0], [1.0, 1.0]),
            ([-half.x, half.y, half.z], [0.0, 0.0, 1.0], [0.0, 1.0]),
            // Back (Z-)
            ([-half.x, -half.y, -half.z], [0.0, 0.0, -1.0], [0.0, 0.0]),
            ([half.x, -half.y, -half.z], [0.0, 0.0, -1.0], [1.0, 0.0]),
            ([half.x, half.y, -half.z], [0.0, 0.0, -1.0], [1.0, 1.0]),
            ([-half.x, half.y, -half.z], [0.0, 0.0, -1.0], [0.0, 1.0]),
            // Left (X-)
            ([-half.x, -half.y, -half.z], [-1.0, 0.0, 0.0], [0.0, 0.0]),
            ([-half.x, half.y, -half.z], [-1.0, 0.0, 0.0], [1.0, 0.0]),
            ([-half.x, half.y, half.z], [-1.0, 0.0, 0.0], [1.0, 1.0]),
            ([-half.x, -half.y, half.z], [-1.0, 0.0, 0.0], [0.0, 1.0]),
            // Right (X+)
            ([half.x, -half.y, -half.z], [1.0, 0.0, 0.0], [0.0, 0.0]),
            ([half.x, half.y, -half.z], [1.0, 0.0, 0.0], [1.0, 0.0]),
            ([half.x, half.y, half.z], [1.0, 0.0, 0.0], [1.0, 1.0]),
            ([half.x, -half.y, half.z], [1.0, 0.0, 0.0], [0.0, 1.0]),
        ];

        for (p_local, n_local, u) in local_vertices {
            let p_vec = Vec3::from(p_local);
            let n_vec = Vec3::from(n_local);
            let p_final = *pos + *rot * p_vec;
            let n_final = *rot * n_vec;
            positions.push([p_final.x, p_final.y, p_final.z]);
            normals.push([n_final.x, n_final.y, n_final.z]);
            uvs.push(u);
        }

        for face in 0..6 {
            let base = offset + face * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    mesh
}
