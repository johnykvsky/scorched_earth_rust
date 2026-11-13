mod ai;
mod game;
mod gfx;
mod terrain;

use macroquad::prelude::*;

fn conf() -> Conf {
    Conf {
        window_title: "Scorched Earth (Rust tribute)".to_string(),
        window_width: gfx::W * 2,
        window_height: gfx::H * 2,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    rand::srand((miniquad::date::now() * 1000.0) as u64);
    let mut fb = gfx::Fb::new();
    let tex = Texture2D::from_image(&fb.img);
    tex.set_filter(FilterMode::Nearest);
    let mut game = game::Game::new();

    loop {
        let dt = get_frame_time().min(1.0 / 30.0);
        if !game.update(dt) {
            break;
        }
        game.draw(&mut fb);
        tex.update(&fb.img);

        // Integer scaling when possible keeps pixels crisp; letterbox the rest.
        clear_background(BLACK);
        let fit = (screen_width() / gfx::W as f32).min(screen_height() / gfx::H as f32);
        let s = if fit >= 1.0 { fit.floor() } else { fit };
        let (w, h) = (gfx::W as f32 * s, gfx::H as f32 * s);
        draw_texture_ex(
            &tex,
            ((screen_width() - w) / 2.0).floor(),
            ((screen_height() - h) / 2.0).floor(),
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() },
        );
        next_frame().await;
    }
}
