# Rust Quiz 3 Feedback

This answer is not correct yet.

## Main Correction

`&T` and `&mut T` are **references to a value of type `T`**. They are not specifically references
to slices.

Examples:

- `&i32` is a shared reference to an integer
- `&String` is a shared reference to a `String`
- `&[i32]` is a shared reference to a slice

So "slice" is only relevant when `T` itself is a slice type like `[i32]` or `str`.

## The Real Difference

### `&T`

- shared borrow
- read-only through that reference
- many `&T` references may exist at the same time

### `&mut T`

- exclusive mutable borrow
- allows mutation through that reference
- while it exists, you generally cannot also have other active references to the same value

So `&mut T` is **not** a shared reference. It is the opposite: an exclusive borrow.

## In the Terms I Asked For

### 1. Aliasing

- `&T`: multiple aliases are allowed
- `&mut T`: exclusive alias only

### 2. Mutation

- `&T`: cannot mutate through it
- `&mut T`: can mutate through it

### 3. What the borrow checker is trying to prevent

The borrow checker is trying to prevent:

- data races
- mutation through one alias while another alias is reading
- iterator/reference invalidation
- more generally, unsound aliasing of mutable state

This is often summarized as:

- either many readers
- or one writer

but not both at the same time.

## Better Answer

`&T` is a shared immutable reference to a value of type `T`, so multiple aliases can exist at once
but you cannot mutate through them. `&mut T` is an exclusive mutable reference, so it allows
mutation but requires unique access for the duration of the borrow. The borrow checker enforces this
to prevent unsafe aliasing, especially read/write conflicts and data races.

## Grade

Roughly: `D`

You got the immutability of `&T` right, but both references were described as slices, and `&mut T`
was incorrectly described as shared.
