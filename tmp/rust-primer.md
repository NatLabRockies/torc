# Rust Primer for Experienced Programmers

This is a language-focused Rust guide for someone who already understands compilers, type systems,
memory models, and common programming paradigms. It is not a beginner's introduction to programming.
The goal is to give you the Rust mental model quickly, then map that model to the syntax and runtime
properties you will actually encounter.

## What Rust Is Optimizing For

Rust is trying to give you:

- C/C++-class control over memory layout and performance
- A much stronger static safety story than C/C++
- Predictable resource management without a tracing garbage collector
- High-level abstractions that usually compile away

The core trade is simple: Rust moves a large amount of correctness work into the type system and
borrow checker. You write more explicit code around aliasing, mutation, and ownership so that the
compiler can reject whole classes of bugs before runtime.

## The First Mental Model

If you come from C++, Java, Python, Go, or functional languages, the most important thing to reset
is this:

- Variables own values by default
- Assignment often moves rather than copies
- Aliasing and mutation are tightly controlled
- Destruction is deterministic via scope-based `Drop`
- Sum types and pattern matching are central, not peripheral

Rust is not "object-oriented with extra steps". It is closer to a systems language with algebraic
data types, trait-based polymorphism, and affine ownership.

## Program Structure

At the top level you work with:

- `crate`: a compilation unit and package-level module tree
- `mod`: a module
- `use`: import names into scope
- `fn`: functions
- `struct`, `enum`, `union`: data types
- `trait`: shared behavior constraints
- `impl`: method or trait implementation blocks
- `const` and `static`: compile-time and static storage items

Example:

```rust
use std::collections::HashMap;

pub struct Cache {
    entries: HashMap<String, usize>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: usize) {
        self.entries.insert(key, value);
    }
}
```

## Variables, Bindings, and Mutability

Bindings are immutable by default:

```rust
let x = 10;
let mut y = 20;
y += x;
```

Important distinction:

- `let mut x` means the binding can be used to mutate the value
- It does not mean the object is universally mutable through any alias

Shadowing is common and idiomatic:

```rust
let path = "data.txt";
let path = std::path::PathBuf::from(path);
```

That is often preferred over carrying one mutable variable through multiple transformations.

## Primitive Types and Built-In Data

Signed integers: `i8`, `i16`, `i32`, `i64`, `i128`, `isize`

Unsigned integers: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`

Floating point: `f32`, `f64`

Other primitives:

- `bool`
- `char`: a Unicode scalar value, not an 8-bit byte
- `str`: dynamically sized UTF-8 string slice type, usually seen as `&str`
- Tuples: `(T, U, V)`
- Arrays: `[T; N]`
- Slices: `&[T]`, `&mut [T]`

String types:

- `String`: owned, growable UTF-8 buffer
- `&str`: borrowed string slice

Example:

```rust
let s1: &str = "hello";
let mut s2: String = String::from("world");
s2.push('!');
```

## Operators

Most familiar operators exist:

| Category | Operators |
| --- | --- |
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Comparison | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Boolean | `&&`, `||`, `!` |
| Bitwise | `&`, `|`, `^`, `<<`, `>>`, `!` |
| Assignment | `=`, `+=`, `-=`, `*=`, `/=`, `%=` and bitwise variants |
| Ranges | `a..b`, `a..=b`, `..b`, `..=b`, `a..`, `..` |
| Borrowing | `&x`, `&mut x` |
| Dereference | `*ptr` |
| Error propagation | `?` |

Notes that matter:

- There is no `++` or `--`
- Short-circuiting works as expected for `&&` and `||`
- Integer overflow is checked in debug builds and wraps in optimized builds unless you use checked,
  saturating, or wrapping APIs explicitly
- `==` and ordering operators depend on trait implementations like `PartialEq` and `Ord`

## Control Flow

`if`, `match`, `while`, `loop`, and `for` are expressions or statements depending on context.

`if` is an expression:

```rust
let sign = if x >= 0 { 1 } else { -1 };
```

`loop` can return a value:

```rust
let result = loop {
    if done() {
        break 42;
    }
};
```

`for` iterates over the `IntoIterator` protocol:

```rust
for item in items {
    println!("{item}");
}
```

`match` is exhaustive and central to Rust style:

```rust
match maybe_number {
    Some(n) if n > 0 => println!("positive"),
    Some(_) => println!("non-positive"),
    None => println!("missing"),
}
```

## Functions

Function syntax is straightforward:

```rust
fn add(x: i32, y: i32) -> i32 {
    x + y
}
```

Notes:

- Parameter types are mandatory
- Return type is introduced with `->`
- The final expression is returned implicitly
- `return expr;` is still available for early returns
- `!` is the never type for functions that do not return

Example:

```rust
fn fail(msg: &str) -> ! {
    panic!("{msg}");
}
```

## "Classes" in Rust

Rust does not have classes or inheritance in the Java/C++ sense.

Instead, you compose three things:

- `struct`: data layout
- `impl`: inherent methods
- `trait`: shared interface and polymorphic behavior

Example:

```rust
struct Counter {
    value: usize,
}

impl Counter {
    fn new() -> Self {
        Self { value: 0 }
    }

    fn increment(&mut self) {
        self.value += 1;
    }

    fn get(&self) -> usize {
        self.value
    }
}
```

This is the closest basic analogue to a class with instance methods.

Associated functions are methods without a `self` receiver. `Counter::new()` is one.

## `self`, `&self`, and `&mut self`

Method receivers are a concise way to express ownership and mutability requirements:

- `self`: method consumes the receiver
- `&self`: shared borrow, read-only access
- `&mut self`: exclusive borrow, mutable access

Example:

```rust
impl String {
    fn len(&self) -> usize { ... }
    fn push(&mut self, ch: char) { ... }
    fn into_bytes(self) -> Vec<u8> { ... }
}
```

That distinction is a large part of Rust's API design vocabulary.

## Structs

Three common forms:

```rust
struct Point {
    x: f64,
    y: f64,
}

struct Color(u8, u8, u8);

struct Marker;
```

Update syntax:

```rust
let p1 = Point { x: 1.0, y: 2.0 };
let p2 = Point { x: 3.0, ..p1 };
```

This moves or copies the unspecified fields from `p1` depending on their types.

## Enums

Enums are one of Rust's most important features. They are full algebraic sum types, not just tagged
integers.

```rust
enum Message {
    Quit,
    Write(String),
    Move { x: i32, y: i32 },
    ChangeColor(u8, u8, u8),
}
```

`Option<T>` and `Result<T, E>` are standard-library enums and define large parts of normal Rust
control flow.

## Pattern Matching

Patterns appear in `match`, `if let`, `while let`, function parameters, and destructuring `let`.

```rust
let (x, y) = (3, 4);

if let Some(name) = maybe_name {
    println!("{name}");
}
```

Destructuring is pervasive and worth learning well. It is one of Rust's core readability tools.

## Ownership

Ownership is the defining Rust feature.

Rules:

1. Every value has one logical owner
2. Moving ownership invalidates the old binding
3. When the owner goes out of scope, the value is dropped

Example:

```rust
let s = String::from("abc");
let t = s;
// s is now invalid; ownership moved to t
```

For `Copy` types like integers, the same syntax performs a bitwise copy:

```rust
let a = 5;
let b = a; // both remain usable
```

`Copy` is intentionally limited to cheap, trivially copyable types. `String`, `Vec<T>`, and most
resource-owning types are move-only.

## Borrowing

You usually do not pass ownership everywhere. You borrow:

- `&T`: shared borrow
- `&mut T`: exclusive mutable borrow

The key aliasing rule is:

- Either many shared references
- Or one mutable reference
- But not both at the same time

Example:

```rust
fn sum(xs: &[i32]) -> i32 {
    xs.iter().sum()
}

fn append_zero(xs: &mut Vec<i32>) {
    xs.push(0);
}
```

This is how Rust prevents data races and a large class of memory errors statically.

## Lifetimes

Lifetimes are not "how long an object lives" in the runtime sense. They are mostly a static model
of reference validity and outliving relationships.

Example:

```rust
fn longer<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() >= y.len() { x } else { y }
}
```

This says the returned reference is valid for at most the shorter of the two input borrow regions.

Most common cases are handled by lifetime elision, so explicit lifetimes show up mainly when the
relationship between input and output references would otherwise be ambiguous.

## The Borrow Checker in Practice

The borrow checker is enforcing a conservative approximation of safe aliasing.

What it is trying to prevent:

- Use-after-free
- Double free
- Iterator invalidation through mutation
- Data races
- Dangling references

Example of a rejected pattern:

```rust
let mut v = vec![1, 2, 3];
let first = &v[0];
v.push(4); // could reallocate
println!("{first}");
```

The compiler refuses this because `push` may invalidate the earlier reference.

## Heap Layout and Indirection

Several standard types are important because they define common allocation and ownership patterns:

- `Box<T>`: unique heap allocation for `T`
- `Vec<T>`: growable contiguous heap buffer
- `String`: `Vec<u8>` with UTF-8 invariants
- `Rc<T>`: single-threaded reference counting
- `Arc<T>`: thread-safe atomic reference counting
- `Cell<T>` and `RefCell<T>`: interior mutability in single-threaded contexts
- `Mutex<T>` and `RwLock<T>`: synchronized shared mutation across threads

If you need graph-like shared ownership, `Rc<T>` or `Arc<T>` is often the signal that you are
leaving Rust's default tree-like ownership world.

## Traits

Traits are Rust's mechanism for shared behavior, ad hoc polymorphism, and many operator or protocol
definitions.

```rust
trait Area {
    fn area(&self) -> f64;
}

impl Area for Point {
    fn area(&self) -> f64 {
        0.0
    }
}
```

Common roles traits play:

- Interface definitions
- Generic constraints
- Operator overloading
- Iteration and conversion protocols
- Marker properties like `Send` and `Sync`

Trait bounds:

```rust
fn print_twice<T: std::fmt::Display>(x: T) {
    println!("{x} {x}");
}
```

Equivalent `where` form:

```rust
fn print_twice<T>(x: T)
where
    T: std::fmt::Display,
{
    println!("{x} {x}");
}
```

## Generics and Monomorphization

Rust generics are usually monomorphized: the compiler generates specialized code per concrete type,
similar to C++ templates.

Benefits:

- Zero-cost abstraction in many cases
- Static dispatch by default

Costs:

- Larger binaries
- Longer compile times

If you need dynamic dispatch, use trait objects like `&dyn Trait` or `Box<dyn Trait>`.

## Trait Objects and Dynamic Dispatch

Example:

```rust
fn draw_all(shapes: &[Box<dyn Draw>]) {
    for shape in shapes {
        shape.draw();
    }
}
```

A trait object is a fat pointer: data pointer plus vtable pointer. It enables runtime dispatch but
usually requires indirection and loses some optimization opportunities.

Use it when you need heterogeneity or late binding, not by default.

## Error Handling

Rust separates recoverable and unrecoverable failure:

- `Result<T, E>` for recoverable errors
- `panic!` for invariant violations or truly unrecoverable conditions

Example:

```rust
fn parse_port(s: &str) -> Result<u16, std::num::ParseIntError> {
    s.parse()
}
```

The `?` operator is central:

```rust
fn load_config(path: &std::path::Path) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}
```

`?` propagates early on `Err`, using conversions through `From` when needed.

## Collections

The standard library's most common collections are:

- `Vec<T>`: dynamic array
- `HashMap<K, V>`: hash table
- `HashSet<T>`: set
- `BTreeMap<K, V>` and `BTreeSet<T>`: ordered tree-based collections
- `VecDeque<T>`: double-ended queue

Example:

```rust
use std::collections::HashMap;

let mut counts = HashMap::new();
counts.insert("a", 1);
counts.entry("a").and_modify(|n| *n += 1).or_insert(1);
```

The `entry` API is a standard Rust pattern worth learning early.

## Iterators

Iterator-heavy code is idiomatic, lazy, and usually efficient.

```rust
let squares_of_even: Vec<i32> = (0..10)
    .filter(|n| n % 2 == 0)
    .map(|n| n * n)
    .collect();
```

This style is not just functional decoration. The iterator traits enable fusion and abstraction
without forcing intermediate allocations in many cases.

Three methods matter constantly:

- `.iter()` yields `&T`
- `.iter_mut()` yields `&mut T`
- `.into_iter()` consumes the collection and yields owned items

Knowing which one you want is often an ownership question.

## Closures

Closures infer parameter and capture behavior. They can capture by shared borrow, mutable borrow, or
move depending on use.

```rust
let factor = 10;
let scale = |x| x * factor;
```

If you need to force capture by value, use `move`:

```rust
let values = vec![1, 2, 3];
let f = move || values.len();
```

Closure traits:

- `Fn`: callable by shared reference
- `FnMut`: needs mutable access to captured state
- `FnOnce`: consumes captured state

## Methods, Traits, and UFCS

Method call syntax:

```rust
obj.method(arg)
```

can desugar conceptually to:

```rust
Type::method(&obj, arg)
```

or similar receiver adjustments through auto-borrow and auto-deref.

If method names are ambiguous across traits, use fully qualified syntax:

```rust
<Type as Trait>::method(&value)
```

## References, Slices, and DSTs

Rust has dynamically sized types such as `str`, `[T]`, and trait objects. These cannot usually live
by value in local variables because their size is not known at compile time.

You usually handle them behind pointers:

- `&str`
- `&[T]`
- `Box<dyn Trait>`

These are often fat pointers carrying metadata like length or vtable pointer.

## Macros

Rust has both declarative and procedural macros.

Common declarative macros:

- `println!`
- `vec!`
- `format!`
- `matches!`

Macros are used because some abstractions need syntax transformation rather than ordinary function
calls. They are important in Rust, but they are not the core language model.

## Concurrency Model

Rust's concurrency story is built on the ownership system.

- `Send`: a type can be transferred to another thread
- `Sync`: shared references to the type are thread-safe

These are auto traits derived structurally unless blocked by internal fields.

Example:

```rust
use std::sync::{Arc, Mutex};
use std::thread;

let counter = Arc::new(Mutex::new(0));

for _ in 0..4 {
    let counter = Arc::clone(&counter);
    thread::spawn(move || {
        let mut guard = counter.lock().unwrap();
        *guard += 1;
    });
}
```

The type system prevents non-thread-safe sharing from compiling.

## `unsafe`

Rust is not "always safe"; it is "safe by default".

`unsafe` allows operations the compiler cannot verify, such as:

- Dereferencing raw pointers
- Calling unsafe functions
- Accessing mutable statics
- Implementing unsafe traits

Important point:

- `unsafe` does not turn off the borrow checker globally
- It creates a small region where you assert extra invariants manually

Good Rust keeps `unsafe` small, documented, and encapsulated behind safe APIs.

## Common Standard Derives

You will see these frequently:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UserId(String);
```

Useful derives include:

- `Debug`
- `Clone`
- `Copy`
- `PartialEq`, `Eq`
- `PartialOrd`, `Ord`
- `Hash`
- `Default`

They generate trait implementations when the fields support them.

## Modules and Visibility

Visibility is private-by-default.

```rust
mod parser {
    pub struct Ast;

    pub fn parse() -> Ast {
        Ast
    }
}
```

Useful visibility forms:

- `pub`
- `pub(crate)`
- `pub(super)`
- `pub(in path)`

This lets you expose APIs with fairly fine control.

## A Small Syntax Cheat Sheet

### Struct and impl

```rust
pub struct User {
    pub id: u64,
    name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
```

### Enum and match

```rust
enum Status {
    Ready,
    Running,
    Failed(String),
}

fn describe(status: &Status) -> &'static str {
    match status {
        Status::Ready => "ready",
        Status::Running => "running",
        Status::Failed(_) => "failed",
    }
}
```

### Generic function

```rust
fn largest<T: Ord>(xs: &[T]) -> Option<&T> {
    xs.iter().max()
}
```

### Result-returning function

```rust
fn read_count(path: &std::path::Path) -> Result<usize, std::io::Error> {
    let text = std::fs::read_to_string(path)?;
    Ok(text.trim().parse().unwrap_or(0))
}
```

## Translating Common Expectations from Other Languages

If you expect classes:

- Use `struct` + `impl` + `trait`

If you expect inheritance:

- Prefer composition and trait bounds

If you expect `null`:

- Use `Option<T>`

If you expect exceptions:

- Use `Result<T, E>` and `?`

If you expect interfaces:

- Use traits

If you expect GC-managed shared mutable objects:

- Reconsider the design first
- Otherwise use `Rc<RefCell<T>>` for single-threaded code or `Arc<Mutex<T>>` for multithreaded code

## What Usually Feels Strange at First

- You must choose ownership intentionally in function signatures
- Mutation and aliasing cannot be hand-waved away
- Many APIs are expressed in terms of enums and pattern matching
- Trait bounds appear everywhere in generic code
- The compiler is often teaching you a real invariant, not just being difficult

Once the model clicks, much of Rust's syntax stops feeling unusual. The language is not complicated
because of surface syntax; it is complicated because it makes memory, aliasing, and error-handling
semantics explicit.

## Practical Learning Order

If you want to become effective quickly, learn in this order:

1. Ownership, borrowing, and moves
2. `Option`, `Result`, and pattern matching
3. Structs, enums, and `impl`
4. Traits and generics
5. Iterators and closures
6. Lifetimes as reference relationships
7. Smart pointers and interior mutability
8. Concurrency traits and synchronization types
9. `unsafe` only after the safe model is solid

## One-Sentence Summary

Rust is a statically compiled systems language where ownership and borrowing are first-class parts
of the type system, enabling memory safety, data-race freedom, and high-performance abstractions
without a garbage collector.
