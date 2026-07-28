//! v0 feature demo: draws text, one shape of each kind (including a filled closed
//! path), and a moving sprite, then renders two frames via the kitty backend.
//!
//! Run in a real kitty-compatible terminal (protocol version 0.20.0+) to visually
//! verify: `cargo run -p guiltty-examples --bin demo`.
//!
//! Does not demonstrate viewport regions, zoom, or scroll/pan -- those are out of
//! v0's current scope (see tasks/plan.md).

use guiltty::{
    Backend, Bitmap, Canvas, Color, Fill, KittyBackend, Point, Shape, Sprite, TextStyle,
};

const SPRITE_PNG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sprite.png");

fn main() {
    let mut canvas = Canvas::new(400, 200);

    canvas.draw_text("GUILTTY V0 DEMO", Point::new(10, 10), &TextStyle::default());

    canvas.draw_shape(
        &Shape::line(Point::new(10, 40), Point::new(150, 40)),
        Fill::solid(Color::rgb(200, 200, 200)),
    );
    canvas.draw_shape(
        &Shape::rect(Point::new(10, 60), 60, 30),
        Fill::solid(Color::rgb(220, 60, 60)),
    );
    canvas.draw_shape(
        &Shape::circle(Point::new(120, 75), 20),
        Fill::solid(Color::rgb(60, 160, 220)),
    );
    canvas.draw_shape(
        &Shape::ellipse(Point::new(200, 75), 30, 15),
        Fill::solid(Color::rgb(80, 200, 120)),
    );
    canvas.draw_shape(
        &Shape::triangle(
            Point::new(250, 90),
            Point::new(280, 60),
            Point::new(310, 90),
        ),
        Fill::solid(Color::rgb(230, 180, 40)),
    );
    // A closed path is filled solid (not just stroked) since tasks/plan.md's T2 --
    // a five-point star-ish polygon, exercising the even-odd fill rule.
    canvas.draw_shape(
        &Shape::path(
            vec![
                Point::new(360, 50),
                Point::new(370, 75),
                Point::new(395, 75),
                Point::new(375, 90),
                Point::new(383, 115),
                Point::new(360, 100),
                Point::new(337, 115),
                Point::new(345, 90),
                Point::new(325, 75),
                Point::new(350, 75),
            ],
            true,
        ),
        Fill::solid(Color::rgb(200, 100, 220)),
    );

    // Sprite: a real loaded image (tasks/plan.md's T3) rather than a solid placeholder,
    // now that Bitmap::from_file exists.
    let bitmap = Bitmap::from_file(SPRITE_PNG).expect("bundled sprite asset should load");
    let mut sprite = Sprite::new(bitmap, Point::new(10, 140));
    canvas.draw_sprite(&mut sprite);

    let mut backend = KittyBackend::new();
    backend
        .present(&canvas)
        .expect("first present should succeed");

    // Move the sprite and render a second frame -- a single present() call after an
    // in-memory move wouldn't actually prove movement or background preservation, so
    // both frames are required (see this task's own acceptance criteria).
    sprite.move_to(Point::new(370, 140));
    canvas.draw_sprite(&mut sprite);
    backend
        .present(&canvas)
        .expect("second present should succeed");
}
