use macroquad::prelude::*;

mod world;
mod game;
mod entities;

#[macroquad::main("Soul Symphony")]
async fn main() {
    loop {
        clear_background(BLACK);

        draw_text("Soul Symphony: Fresh Start", 20.0, 20.0, 30.0, DARKGRAY);

        next_frame().await
    }
}
