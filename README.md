
### Rust Interop

```ts
rust dependency macroquad = "0.4"

use crate::macroquad::prelude
use crate::macroquad::math::Vec2
use std::math

obj Triangle:
    v1: Vec2
    v2: Vec2
    v3: Vec2

#[macroquad::main("window")]
async fn main():
    var t = Triangle { 
		v1: prelude::vec2(30.0, 120.0),
		v2: prelude::vec2(120.0, 30.0),
		v3: prelude::vec2(120.0, 90.0)
	}

    while true:
        prelude::clear_background(prelude::BLACK)
        prelude::draw_triangle(t.v1, t.v2, t.v3, prelude::WHITE)
        prelude::next_frame().await
```
