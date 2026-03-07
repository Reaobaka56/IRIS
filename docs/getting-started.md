# Getting Started with IRIS

## Installation

IRIS ships as a single binary. Copy it to a directory on your `PATH`:

```bash
# Windows (MSYS2 / Git Bash)
cp iris.exe /c/msys64/ucrt64/bin/

# macOS / Linux
cp iris /usr/local/bin/
```

Verify the installation:

```bash
iris --version
```

---

## Your First Program

Create `hello.iris`:

```iris
def main() -> i64 {
    print("Hello, IRIS!");
    0
}
```

Run it:

```bash
iris run hello.iris
```

---

## Variables and Types

```iris
def main() -> i64 {
    val name = "Alice"       // immutable string
    var count = 0            // mutable integer
    val pi: f64 = 3.14159

    count = count + 1
    print(concat("Hello, ", name));
    print(count);
    0
}
```

---

## Functions

```iris
def factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}

def main() -> i64 {
    print(factorial(10));
    0
}
```

---

## Lists and Iteration

```iris
def main() -> i64 {
    val nums = list()
    push(nums, 10);
    push(nums, 20);
    push(nums, 30);

    var i = 0
    while i < list_len(nums) {
        print(list_get(nums, i));
        i = i + 1
    }
    0
}
```

Or with a range loop:

```iris
for i in 0..10 {
    print(i);
}
```

---

## Using the Standard Library

```iris
bring std.math
bring std.string
bring std.time

def main() -> i64 {
    val t0  = stopwatch_start()
    val g   = gcd(48, 18)
    val dur = stopwatch_stop(t0)

    print(concat("GCD = ", to_str(g)));
    print(concat("Took: ", format_duration(dur)));
    0
}
```

---

## Records (Structs)

```iris
record Person {
    name: str,
    age:  i64
}

def greet(p: Person) -> str {
    concat("Hello, ", concat(p.name, "!"))
}

def main() -> i64 {
    val alice = Person { name: "Alice", age: 30 }
    print(greet(alice));
    0
}
```

---

## Pattern Matching

```iris
choice Shape { Circle, Square, Triangle }

def describe(s: Shape) -> str {
    when s {
        Shape.Circle   => "round",
        Shape.Square   => "four sides",
        Shape.Triangle => "three sides"
    }
}
```

---

## Writing Tests

Create `my_tests.iris`:

```iris
bring std.testing

def test_addition() -> bool {
    assert_eq(1 + 1, 2, "1+1=2")
}

def test_strings() -> bool {
    assert_str_eq(concat("a", "b"), "ab", "concat")
}
```

Run tests:

```bash
iris test my_tests.iris
```

Output:

```
running tests in my_tests.iris

  test test_addition ... PASS (0.05ms)
  test test_strings  ... PASS (0.02ms)

test result: ok. 2 passed; 0 failed; 0 ignored
```

---

## Reading and Writing Files

```iris
bring std.fs

def main() -> i64 {
    val ok = write_text("output.txt", "Hello from IRIS!\n")
    val content = read_text("output.txt")
    print(content);
    0
}
```

---

## Compiling to Native Binary

```bash
iris build program.iris -o program
./program
```

Requires LLVM (`clang`) on your PATH.

---

## VS Code Extension

1. Open VS Code.
2. Install the **IRIS Language** extension from the Extensions panel (or `.vsix` file).
3. Open any `.iris` file to get syntax highlighting, completions, error diagnostics, and go-to-definition.

---

## Project Layout (iris.toml)

For multi-file projects, create an `iris.toml`:

```toml
[package]
name    = "my-project"
version = "0.1.0"

[dependencies]
# local path dependency
# my-lib = { path = "../my-lib" }
```

```bash
iris pkg init           # create iris.toml
iris pkg install        # install deps
iris pkg build          # build project
iris pkg run            # run main
```

---

## Next Steps

- [Language Reference](language-reference.md) — complete syntax and type system
- [Standard Library Reference](stdlib-reference.md) — all stdlib modules
- [Examples](../examples/) — sample programs
