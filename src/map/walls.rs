

use super::types::Room;



// Python parity: _get_room_anchor logic
pub(crate) fn get_room_anchor(room: &Room, tx: f32, ty: f32) -> (f32, f32, &'static str, f32) {
    let dx = tx - (room.x + room.w * 0.5);
    let dy = ty - (room.y + room.h * 0.5);

    if dx.abs() >= dy.abs() {
        let y = ty.clamp(room.y + 1.2, room.y + room.h - 1.2);
        if dx >= 0.0 {
            (room.x + room.w, y, "right", y)
        } else {
            (room.x, y, "left", y)
        }
    } else {
        let x = tx.clamp(room.x + 1.2, room.x + room.w - 1.2);
        if dy >= 0.0 {
            (x, room.y + room.h, "top", x)
        } else {
            (x, room.y, "bottom", x)
        }
    }
}


