mod ai;
mod controls;
mod earth;
mod economy;
mod energy;
mod environment;
mod game;
mod gfx;
mod guidance;
mod hall_of_fame;
mod inventory;
mod landscape;
mod liquid;
mod modes;
mod movement;
mod physics;
mod play;
mod presentation;
mod saved_game;
mod settings;
mod shields;
mod shop;
mod sound;
mod speech;
mod system;
mod tank;
mod terrain;
mod ui;
mod weapons;
mod window;

use macroquad::prelude::*;

fn conf() -> Conf {
    Conf {
        window_title: "Scorched Earth (Rust tribute)".to_string(),
        window_width: gfx::W * 2,
        window_height: gfx::H * 2,
        window_resizable: true,
        platform: miniquad::conf::Platform {
            linux_wm_class: window::startup_class(),
            ..Default::default()
        },
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
    let mut audio = match sound::Player::load().await {
        Ok(audio) => Some(audio),
        Err(error) => {
            eprintln!("{error}; continuing without sound.");
            None
        }
    };

    let mut fullscreen = false;
    let mut startup_activation = true;
    loop {
        if is_key_pressed(KeyCode::F11) {
            game.toggle_fullscreen();
        }
        if fullscreen != game.fullscreen() {
            fullscreen = game.fullscreen();
            set_fullscreen(fullscreen);
        }
        let dt = get_frame_time().min(1.0 / 30.0);
        if !game.update(dt) {
            break;
        }
        let sound = game.take_sound_frame();
        if let Some(audio) = &mut audio {
            audio.update(sound, dt);
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
        if startup_activation {
            window::activate_on_startup();
            startup_activation = false;
        }
    }
}
