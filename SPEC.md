This is a powerful direction. By using & and &mut for strict borrows and * for managed references, we create a clear
  "Speed vs. Flexibility" hierarchy while keeping the syntax minimal.

  Here is how the Transparent Borrowing + Managed Fallback system would look in practice:

  1. The Syntax
  We maintain three modes for any variable or parameter:

  ┌─────────┬────────────────────┬────────────────────┬─────────────────────────────┐
  │ Mode    │ Solar Syntax       │ Rust Transpilation │ Character                   │
  ├─────────┼────────────────────┼────────────────────┼─────────────────────────────┤
  │ Owned   │ var p = Point{...} │ Point              │ Affine/Unique (Fastest)     │
  │ Borrow  │ fn draw(p: &Point) │ &Point             │ Temporary access (Fast)     │
  │ Managed │ fn save(p: *Point) │ Rc<RefCell<Point>> │ Shared ownership (Fallback) │
  └─────────┴────────────────────┴────────────────────┴─────────────────────────────┘

  2. The "Sugared" Experience
  The goal is that you almost never write * manually. You write your code with & and &mut, and Solar only forces (or
  warns) you to use * when the borrow checker would otherwise fail.

    1 obj Sprite:
    2     pos: Point
    3
    4 # Function expects a fast, raw borrow
    5 fn update_position(p: &mut Point):
    6     p.x += 1.0
    7
    8 fn main():
    9     var mut p = Point{x: 10, y: 10}
   10
   11     # 1. Standard fast path
   12     update_position(&mut p) # Transpiles to: update_position(&mut p)
   13
   14     # 2. The "Headache" Case: Multiple long-lived owners
   15     # Imagine a game where multiple Sprites share the same Position object.
   16     var s1 = Sprite{ pos: &p }
   17     var s2 = Sprite{ pos: &p }
   18
   19     # In Rust, this would error (lifetime of p vs s1/s2).
   20     # In Solar, the compiler says:
   21     # "Warning: &p escapes local scope. Promoting 'p' to Managed (*Point)."

  3. Under the Hood: The Transpiler "Fixup"
  When that warning triggers, the transpiler rewrites the local scope:

  Original (Solar):

   1 var mut p = Point{x: 10, y: 10}
   2 update_position(&mut p)

  Transpiled (Rust) after promotion:

   1 // p is promoted to Rc<RefCell>
   2 let p = Rc::new(RefCell::new(Point { x: 10, y: 10 }));
   3
   4 // The call to update_position was expecting &mut Point.
   5 // Solar inserts the transparent borrow:
   6 update_position(&mut *p.borrow_mut());

  4. Why this is "Cleaner" than shared or ref
   1. Uniform Access: In Solar, you just type p.x. You don't care if p is a Point, a &Point, or a *Point. The transpiler
      handles the .borrow() or deref internally.
   2. Explicit Intent with *: You only use *Point in your struct definitions when you know you want a shared,
      reference-counted object (like a Scene Graph node).
   3. Rust Crate Compatibility: Since functions stay as &Point, you can pass your *Point (managed) into a Macroquad
      draw_circle function, and Solar just passes the .borrow(). It "just works."

  The Implementation Strategy
  To build this, we need to add a "Promotion Pass" to the SemanticAnalyzer.

  The Plan:
   1. Update AST: Add Type::Managed and ExprKind::PromoteToManaged.
   2. Semantic Analysis: When checking a Borrow, if the lifetime check (a simplified version) fails, mark the variable
      for promotion.
   3. Compiler (Transpiler):
       * If a variable is marked Managed, wrap its allocation in Rc::new(RefCell::new(...)).
       * At every usage site, check if the consumer expects a raw reference. If so, insert .borrow() or .borrow_mut().

  One crucial question to settle before I design the spec:
  If the compiler promotes a variable in main that is passed to fn foo(p: &Point), we can easily insert .borrow() at the
  call site. But what if the compiler decides a function return type needs to be promoted?

  Example:

   1 fn get_player() -> &Point:
   2     return &self.player_instance # Illegal in Rust if returning local or complex ref
  If this promotes, should the function signature automatically change to return *Point? (This would mean the caller
  also needs to treat it as a managed object).
