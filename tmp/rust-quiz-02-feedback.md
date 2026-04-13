# Rust Quiz 2 Feedback

Your answer is pointing at the ownership idea, but it is not precise enough yet.

## Main Correction

In Rust, a move does **not** mean "the reference changed" in the ordinary sense.

More precisely:

- a move transfers ownership of a value from one binding to another
- after the move, the old binding is no longer usable
- for many types, the bits may be copied at machine level, but Rust treats the old binding as
  invalid

So the important thing is not physical relocation in memory. The important thing is change of
**ownership**.

## Your Three Parts

### 1. What happens semantically

Your answer:

> "moving" refers to a the reference to a value changing rather than a copy to a new location

Better:

> A move means ownership of the value is transferred. The old variable loses the right to use the
> value.

Example:

```rust
let s1 = String::from("hello");
let s2 = s1;
// s1 is now invalid
```

This does not mean the heap buffer necessarily moved. It means `s2` is now the owner.

### 2. Which kinds of types are typically moved

Your answer:

> references to values are moved

This is not the right level.

Better:

- non-`Copy` types are moved by default
- common examples are `String`, `Vec<T>`, `Box<T>`, and most resource-owning structs

By contrast:

- `Copy` types like `i32`, `bool`, and many small plain-data types are copied rather than moved in
  the "old binding becomes invalid" sense

References themselves can also be copied or moved depending on the exact type context, but that is
not the main rule Rust means when teaching ownership.

### 3. Why this matters for memory safety

Your instinct here was basically right.

Better:

- if only one owner is responsible for dropping a resource, Rust can prevent double frees
- invalidating the old binding prevents use-after-free through that binding
- explicit ownership transfer makes aliasing and mutation easier to reason about statically

This is one of the foundations of Rust's memory-safety model.

## Better Answer

A move in Rust means ownership of a value is transferred from one binding to another, and the old
binding becomes unusable. This usually applies to non-`Copy` types such as `String`, `Vec<T>`, and
other resource-owning values. It matters for memory safety because Rust can enforce a single clear
owner for destruction, which helps prevent double free, use-after-free, and unsafe aliasing.

## Grade

Roughly: `C+`

You were circling the correct idea, especially on part 3, but you need to replace "reference
changed" with "ownership transferred."
