# Rust Slices

A slice is a non-owning view into a contiguous sequence of data.

In normal Rust code, the two slice forms you use most are:

- `&[T]`: a shared slice of elements of type `T`
- `&mut [T]`: a mutable slice of elements of type `T`
- `&str`: a shared slice of UTF-8 text

The key point is that a slice does not own the underlying storage. It borrows some region of
already-existing data.

## The Mental Model

If `Vec<T>` is the owned buffer, a slice is a borrowed window into that buffer.

```rust
let v = vec![10, 20, 30, 40];
let s: &[i32] = &v[1..3]; // [20, 30]
```

Similarly, if `String` is the owned text buffer, `&str` is a borrowed view into it.

```rust
let text = String::from("hello");
let part: &str = &text[1..4]; // "ell"
```

So conceptually:

- `Vec<T>` owns heap memory
- `&[T]` borrows contiguous elements from it
- `String` owns UTF-8 bytes
- `&str` borrows some UTF-8 range from it

## Why Slices Exist

Slices let Rust separate ownership from access.

That matters because you often want a function to read or modify a sequence without taking ownership
of the whole container.

For example:

```rust
fn sum(xs: &[i32]) -> i32 {
    xs.iter().sum()
}
```

This is flexible because `sum` can accept:

- an array: `&[1, 2, 3]`
- a `Vec<i32>`: `&vec`
- a subrange of either: `&vec[2..5]`

The function does not care who owns the storage. It only needs a valid contiguous view.

## Runtime Representation

A slice is not just a pointer. It is a fat pointer.

For `&[T]`, the runtime value is effectively:

- a pointer to the first element
- a length

For `&str`, it is effectively:

- a pointer to the first byte
- a byte length

That is why slices are dynamically sized types in the type-system sense: the length is known only at
runtime, so you usually handle them behind a reference like `&[T]` or `&str`.

## Borrowing Rules Still Apply

Because a slice is a borrow, it is subject to the normal Rust borrowing rules.

Example:

```rust
let mut v = vec![1, 2, 3];
let s = &v[..];
// v.push(4); // not allowed while s is alive
println!("{s:?}");
```

The reason is that `push` may reallocate the vector, which could invalidate the slice.

Mutable slices require exclusive access:

```rust
fn zero(xs: &mut [i32]) {
    for x in xs {
        *x = 0;
    }
}
```

`&mut [T]` means "I have exclusive mutable access to this contiguous region for the duration of the
borrow."

## Slices Versus Arrays and Vectors

These are different things:

- `[T; N]`: an array with size known at compile time
- `Vec<T>`: a growable owned heap buffer
- `[T]`: the slice type itself, dynamically sized
- `&[T]`: a borrowed slice

Example:

```rust
let a: [i32; 4] = [1, 2, 3, 4];
let s: &[i32] = &a[1..3];
```

The array owns its elements. The slice just views part of them.

## String Slices Are Byte Slices with UTF-8 Invariants

`&str` is not "a sequence of characters" in the simple fixed-width sense. It is a slice of bytes
that Rust guarantees is valid UTF-8.

That is why string slicing uses byte offsets, not character indices.

```rust
let s = "hello";
let part = &s[1..4]; // "ell"
```

This works because ASCII characters are one byte each. But with non-ASCII text, arbitrary byte
offsets can split a code point and therefore panic.

So with `&str`, you must slice on UTF-8 boundaries.

## Why APIs Prefer Slices

You will see slice-based APIs everywhere because they are:

- non-owning
- zero-copy
- generic over many backing containers
- explicit about contiguity

This is a common Rust signature:

```rust
fn process(bytes: &[u8]) {
    // ...
}
```

It is usually a better API than taking `Vec<u8>` if the function does not need ownership.

## The Short Version

A slice is Rust's standard borrowed representation of "some contiguous part of existing data."

For collections, use `&[T]` or `&mut [T]`.

For strings, use `&str`.

It is one of the main mechanisms Rust uses to let code access data without transferring ownership.
