# Solar (SR) Language: Explicit Intent & 0CA Specification

## Core Philosophy: "Sugaring Rust"
Solar is a high-performance, indentation-based systems language designed as a 1:1 zero-cost abstraction (0CA) over Rust. It eliminates manual lifetime management by providing explicit ownership primitives and a "suggestive" compiler that guides developers toward managed pointers when lifetime issues arise.

## 1. Zero-Cost Abstractions (1:1 with Rust)

| Solar Construct | Rust Representation | Performance |
| :--- | :--- | :--- |
| `obj Name` | `struct Name` | 0CA (Supports `#[derive(...)]`) |
| `enum Name` | `enum Name` | 0CA |
| `trait Name` | `trait Name` | 0CA (Desugars to standard Rust traits) |
| `fn name()` | `fn name()` | 0CA |
| `loop`, `while`, `for` | `loop`, `while`, `for` | 0CA |
| `Result<T, E>` | `Result<T, E>` | 0CA |

## 2. Pointer & Ownership Primitives (The "No-Lifetime" Model)

Solar forbids manual lifetime annotations. Instead, it uses explicit symbols to define how data is stored and shared.

| Solar Type | Rust Underlying Type | Usage |
| :--- | :--- | :--- |
| `T` | `T` | Owned value (Stack/Inline) |
| `&T` | `&T` | Local reference (Read-only) |
| `&mut T` | `&mut T` | Local reference (Mutable) |
| `*box T` | `Box<T>` | Owned heap allocation |
| `*rc T` | `Rc<RefCell<T>>` | Shared ownership, single-threaded mutation |
| `**arc T` | `Arc<RwLock<T>>` | Shared ownership, thread-safe mutation |
| `*const T` | `*const T` | Raw constant pointer (Unsafe) |
| `*mut T` | `*mut T` | Raw mutable pointer (Unsafe) |

### Implicit Dereferencing & Guarding
The compiler automatically handles the "ceremony" of accessing data behind managed pointers:
- `*rc T` access `p.x` -> `p.borrow().x`
- `**arc T` access `p.x` -> `p.read().x`
- `**arc T` assignment `p.x = v` -> `p.write().x = v`

## 3. The "Trust System" (Rust Interop)

Solar allows importing Rust crates and treats their types as "Transparent."
- **Auto-Newtype**: (Planned) Automatically wrap external types if necessary for trait implementations.
- **Trait Preservation**: If `macroquad::Vec2` implements `Add`, Solar allows `v1 + v2` by emitting raw Rust calls.
- **Phantom Types**: External crate types are treated as `Type::Any` during Solar analysis, letting the Rust compiler validate the final implementation.

## 4. Current Implementation Constraints (Refactoring Targets)

1.  **Remove Greedy Borrowing**: The compiler must stop forcing `(&expr).as_val()` everywhere. It should respect the Solar signature.
2.  **Strict Reference Translation**: Solar `&T` must translate exactly to Rust `&T`. Solar `T` (owned) must translate to Rust `T`.
3.  **Primitive Consistency**: Fix `src/codegen/stdlib.rs` to support `as_val` for all primitive types including `()` to handle `Ok(())` returns.
