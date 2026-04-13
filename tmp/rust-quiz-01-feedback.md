# Rust Quiz 1 Feedback

Your answer is partly right, but a few points need correction.

## What You Got Right

- `&str` is a borrowed string slice.
- Taking `&str` in function parameters is usually the better API when ownership is not needed.
- A function taking `&str` can be called with either a `String` or a string literal in many normal
  cases.

## Corrections

### 1. `String` is not "an array of UTF-8 characters"

More precise statement:

- `String` is an owned, growable UTF-8 byte buffer.

Why this matters:

- Rust strings are stored as bytes, not as a random-access array of characters.
- Unicode characters are variable-width in UTF-8.
- Indexing by "character position" is therefore not supported the way it is in many other
  languages.

### 2. `&str` is not just "a slice of some array"

More precise statement:

- `&str` is a borrowed slice of bytes that Rust guarantees is valid UTF-8 text.

That UTF-8 validity guarantee is the important distinction between `&str` and `&[u8]`.

### 3. Mutability is about the borrow type too, not just the original `String`

This part of your answer is not correct as written.

Rust distinguishes:

- `&str`: shared, immutable string slice
- `&mut str`: mutable string slice, rare in normal code

So mutability is not determined only by whether the original `String` binding was declared `mut`.
The reference type also matters.

### 4. Your example syntax is not valid Rust

This:

```rust
let mut x = String("hello");
let &s = x[1::3];
```

is invalid for multiple reasons:

- `String("hello")` is not Rust syntax
- string slicing uses `..`, not `::`
- pattern `let &s = ...` is not what you want here

A valid version would look more like:

```rust
let mut x = String::from("hello");
let s: &str = &x[1..4];
```

## Better Answer

`String` is an owned, growable UTF-8 byte buffer, while `&str` is a borrowed string slice referring
to UTF-8 text owned elsewhere. In APIs, `&str` is usually preferred for inputs because it lets the
function accept either borrowed string data or owned `String` values without taking ownership.

## Grade

Roughly: `B-`

You had the main distinction, but your wording around UTF-8 characters and mutability was too loose,
and the code example was not valid Rust.
