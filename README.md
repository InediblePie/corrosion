# Corrosion

Corrosion is an experimental, embeddable scripting language written in Rust.

It is designed for small, single-threaded scripts inside Rust applications: helper logic, customization, lightweight dynamic processing, and host-controlled imports.

The language is dynamically typed and uses a C/Rust-like syntax with braces, required semicolons for simple statements, first-class functions, reference-based arrays and objects, namespaces, and explicit receiver-style method calls with `->`.

## Status

Corrosion is early-stage and still evolving. The current implementation is an AST interpreter with a lexer, parser, runtime value model, native functions, imports, arrays, objects, functions, control flow, and namespaces.

The API and language details may change while the v0.1 feature set settles.

## Language Preview

```crs
namespace People {
    extern let make_person = fn(name, age) {
        return struct {
            .name: name,
            .age: age,
            .birthday: fn(this) {
                this.age = this.age + 1;
            },
            .to_string: fn(this) {
                return this.name + " " + String(this.age);
            }
        };
    };
}

let person = People.make_person("Colby", 23);
person->birthday();
print(person);

let numbers = [];
numbers->push(1);
numbers->push(2);
numbers->push(3);

for let i = 0; i < numbers.length; i = i + 1 {
    print(numbers[i]);
}

print(10 % 3);
```

## Features

- Source spans and structured lexer, parser, and runtime errors
- Comments: `//` and `/* ... */`
- Primitive values: `null`, bools, ints, floats, and strings
- Reference values: arrays, objects, functions, native functions, and namespaces
- Arithmetic, comparison, boolean logic, string concatenation, and integer modulus
- Variables, assignment, `if` / `else`, `while`, and C-style `for`
- First-class function values with missing args as `null`, ignored extra args, and rest params
- Object literals with `struct { .field: value }`
- Array literals, sized arrays, indexing, slicing, concatenation, and mutation methods
- Explicit method calls: `object->method(arg)`
- Namespaces with `extern let` exports and private internal values
- Host-controlled imports through an `ImportResolver`
- Built-ins: `print`, `String`, `Int`, and `Float`

## Embedding From Rust

```rust
use corrosion::{Engine, Runtime};

fn main() -> Result<(), corrosion::ScriptError> {
    let engine = Engine::new();
    let script = engine.compile(
        r#"
        let numbers = [];
        numbers->push(1);
        numbers->push(2);
        numbers->push(3);

        for let i = 0; i < numbers.length; i = i + 1 {
            print(numbers[i]);
        }
        "#,
    )?;

    let mut runtime = Runtime::new(engine);
    runtime.execute(&script)?;

    for line in runtime.engine.take_output() {
        println!("{line}");
    }

    Ok(())
}
```

Run the included Rust embedding example:

```sh
cargo run --example basic
```

## Examples

Example CRS programs live in `examples/`:

- `hello_world.crs`
- `fizz_buzz.crs`
- `factorial.crs`
- `fibonacci.crs`
- `arrays_and_slices.crs`
- `objects_and_methods.crs`
- `namespaces.crs`
- `conversions.crs`

These examples are also executed by the test suite so they stay valid as the language changes.

## Project Layout

```text
src/
  source/    source spans
  lexer/     tokenization and lexer errors
  parser/    AST definitions and recursive-descent parser
  runtime/   interpreter, values, scopes, arrays, objects, namespaces
  engine/    host-facing compile/import/native-function API
  error.rs   top-level ScriptError

examples/    Rust embedding example and CRS sample programs
tests/       integration tests
```

## Development

Run tests:

```sh
cargo test
```

Check formatting:

```sh
cargo fmt --check
```

Format the project:

```sh
cargo fmt
```

## Current Limitations

- No bytecode VM yet
- No tracing garbage collector yet
- No closures over expired function scopes
- No threading or async runtime
- No exception syntax
- No object field deletion
- No static type system

Corrosion intentionally starts small. The goal is a clear, embeddable scripting runtime before adding heavier language machinery.
