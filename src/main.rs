mod ai;
mod economy;
mod game;
mod gfx;
mod inventory;
mod physics;
mod settings;
mod shop;
mod tank;
mod terrain;
mod ui;
mod weapons;

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
    let path = match settings::config_path(std::env::args_os().skip(1)) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let mut game = game::Game::new();
    game.load_settings(path);

    loop {
        let dt = get_frame_time().min(1.0 / 30.0);
        if !game.update(dt) {
            break;
        }
        game.draw(&mut fb);
        tex.update(&fb.img);

        // Integer scaling when possible keeps pixels crisp; letterbox the rest.
        clear_background(BLACK);
        let view = gfx::Viewport::new(screen_width(), screen_height());
        draw_texture_ex(
            &tex,
            view.x,
            view.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(view.width, view.height)),
                ..Default::default()
            },
        );
        next_frame().await;
    }
}
