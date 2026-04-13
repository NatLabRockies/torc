# What "Full Algebraic Sum Type" Means

When people say Rust `enum`s are "full algebraic sum types", they mean:

- they are not just named integer constants
- each variant can carry different data
- pattern matching can destructure that data
- the compiler tracks the whole set of possible cases
- code using them is usually required to be exhaustive

## The "Sum Type" Part

A sum type means "a value is one of several alternatives".

If a type can be either `A` or `B`, that is a sum:

```text
T = A + B
```

The `+` is not arithmetic execution here. It means a value of `T` is one variant or the other.

Example:

```rust
enum BoolLike {
    False,
    True,
}
```

That is a trivial sum type with two possibilities.

## Why It Is Called "Algebraic"

The "algebraic" part comes from composing types with operations that resemble algebra:

- product types: "and"
- sum types: "or"

A product type is like a tuple or struct:

```rust
struct Point {
    x: f64,
    y: f64,
}
```

This is a product type because a `Point` contains both an `x` and a `y`.

A sum type is like an enum:

```rust
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}
```

A `Shape` is either a `Circle` or a `Rectangle`.

So:

- structs/tuples are products
- enums are sums

## What "Full" Means

In some languages, an enum is just a small integer with names attached:

```c
enum Color {
    RED,
    GREEN,
    BLUE
};
```

That is useful, but limited. Each variant is basically just a tag.

Rust enums are much richer. Each variant can have its own payload and structure:

```rust
enum Message {
    Quit,
    Write(String),
    Move { x: i32, y: i32 },
    ChangeColor(u8, u8, u8),
}
```

This is what people mean by a "full" sum type:

- variants are not just labels
- variants can carry different types of data
- the compiler knows exactly which data belongs to which variant

Each variant is almost like its own constructor for the overall type.

## Why This Matters

Because the enum carries both:

- the tag saying which case you have
- the data specific to that case

you can write code like this:

```rust
match message {
    Message::Quit => println!("quit"),
    Message::Write(text) => println!("write: {text}"),
    Message::Move { x, y } => println!("move to ({x}, {y})"),
    Message::ChangeColor(r, g, b) => println!("rgb({r}, {g}, {b})"),
}
```

The compiler checks that:

- every possible variant is handled
- the payload pattern matches the variant shape
- you do not accidentally read fields that do not exist for the current case

That is substantially stronger than an integer enum plus a side-channel set of fields.

## `Option<T>` Is the Canonical Example

```rust
enum Option<T> {
    None,
    Some(T),
}
```

This says a value is either:

- `None`
- `Some(T)`

That is a real sum type. One branch carries no payload, the other carries a `T`.

Likewise:

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

is a sum type between success and failure.

## Comparison to OOP Class Hierarchies

If you come from Java or C++, a full algebraic sum type often plays the role that a closed class
hierarchy might play.

For example, instead of:

- a base class `Expr`
- subclasses `Literal`, `Add`, `Multiply`

Rust often uses:

```rust
enum Expr {
    Literal(i64),
    Add(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
}
```

That is a closed set of cases known at compile time, and `match` is the standard way to consume it.

This differs from trait objects or inheritance-style polymorphism:

- sum types are closed and compiler-known
- trait objects are open-ended and dispatch through behavior

## Closed World Versus Open World

This is one of the most important distinctions.

An algebraic sum type is usually closed:

- the set of variants is fixed by the enum definition

A trait-based interface is usually open:

- new implementors can exist elsewhere

Use a sum type when:

- the set of cases is known
- each case may need different data
- callers should handle cases explicitly

Use trait-based polymorphism when:

- you want extensibility
- you care more about shared behavior than explicit case analysis

## Why Rust Enums Are So Central

Rust leans heavily on enums because they encode state and control flow precisely.

Examples:

- `Option<T>` instead of `null`
- `Result<T, E>` instead of exceptions
- domain states like `Ready | Running | Failed`
- ASTs, protocol messages, parser outputs, command variants

This is one reason Rust code often feels more explicit than OOP-heavy code: the possible states are
represented directly in the type.

## Short Version

A "full algebraic sum type" is a type whose values can be one of several named variants, where each
variant may carry its own differently shaped data, and where the compiler understands and checks the
entire set of cases.

In Rust, `enum`s are full algebraic sum types.
