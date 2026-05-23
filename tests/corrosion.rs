use corrosion::{
    Engine, Runtime, Value,
    engine::{ImportResolver, ResolvedModule},
    runtime::RuntimeError,
};
use std::{fs, path::Path};

fn run(source: &str) -> Result<(Runtime, Vec<String>), corrosion::ScriptError> {
    let engine = Engine::new();
    let script = engine.compile(source)?;
    let mut runtime = Runtime::new(engine.clone());
    runtime.execute(&script)?;
    let output = runtime.engine.output();
    Ok((runtime, output))
}

fn run_error(source: &str) -> corrosion::ScriptError {
    match run(source) {
        Ok(_) => panic!("expected script to fail"),
        Err(error) => error,
    }
}

#[test]
fn runs_math_variables_and_print() {
    let (_, output) = run(r#"
        let x = 1 + 2 * 3;
        print(x);
        print("age: " + 23);
        print(10 % 3);
        print(10 + 6 % 4 * 2);
        "#)
    .unwrap();

    assert_eq!(output, vec!["7", "age: 23", "1", "14"]);
}

#[test]
fn modulus_requires_ints_and_nonzero_divisor() {
    let float_error = run_error("print(10.5 % 2);");
    assert!(float_error.to_string().contains("integer operands"));

    let zero_error = run_error("print(10 % 0);");
    assert!(zero_error.to_string().contains("modulus by zero"));
}

#[test]
fn runs_control_flow_and_arrays() {
    let (_, output) = run(r#"
        let numbers = [];
        numbers->push(1);
        numbers->push(2);
        numbers->push(3);

        for let i = 0; i < numbers.length; i = i + 1 {
            print(numbers[i]);
        }
        "#)
    .unwrap();

    assert_eq!(output, vec!["1", "2", "3"]);
}

#[test]
fn runs_functions_objects_and_methods() {
    let (_, output) = run(r#"
        let make_person = fn(name, age) {
            return struct {
                .name: name,
                .age: age,
                .to_string: fn(this) {
                    return "Name: " + this.name + ", Age: " + String(this.age);
                },
                .birthday: fn(this) {
                    this.age = this.age + 1;
                }
            };
        };

        let person = make_person("Colby", 23);
        person->birthday();
        print(person);
        "#)
    .unwrap();

    assert_eq!(output, vec!["Name: Colby, Age: 24"]);
}

#[test]
fn supports_rest_parameters() {
    let (_, output) = run(r#"
        let f = fn(first, rest...) {
            print(first);
            print(rest.length);
        };

        f(1, 2, 3, 4);
        "#)
    .unwrap();

    assert_eq!(output, vec!["1", "3"]);
}

#[test]
fn enforces_namespace_privacy() {
    let (_, output) = run(r#"
        namespace Lib {
            extern let public_value = 1;
            let private_value = 2;
        }

        print(Lib.public_value);
        "#)
    .unwrap();

    assert_eq!(output, vec!["1"]);

    let error = run_error(
        r#"
        namespace Lib {
            extern let public_value = 1;
            let private_value = 2;
        }

        print(Lib.private_value);
        "#,
    );

    assert!(error.to_string().contains("private"));
}

#[test]
fn reports_array_bounds_errors() {
    let error = run_error(
        r#"
        let arr = [1, 2];
        print(arr[10]);
        "#,
    );

    assert!(error.to_string().contains("out of bounds"));
}

#[derive(Clone)]
struct TestResolver;

impl ImportResolver for TestResolver {
    fn resolve(&self, _from: Option<&str>, path: &str) -> Result<ResolvedModule, RuntimeError> {
        assert_eq!(path, "people.crs");
        Ok(ResolvedModule {
            id: "people".to_string(),
            source: r#"
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
            "#
            .to_string(),
        })
    }
}

#[test]
fn executes_imports_once() {
    let mut engine = Engine::new();
    engine.set_import_resolver(TestResolver);
    let script = engine
        .compile(
            r#"
            import "people.crs";
            import "people.crs";

            let person = People.make_person("Colby", 23);
            person->birthday();
            print(person);
            "#,
        )
        .unwrap();

    let mut runtime = Runtime::new(engine);
    runtime.execute(&script).unwrap();

    assert_eq!(runtime.engine.output(), vec!["Colby 24"]);
}

#[test]
fn duplicate_let_is_runtime_error() {
    let error = run_error(
        r#"
        let x = 1;
        let x = 2;
        "#,
    );

    assert!(error.to_string().contains("already defined"));
}

#[test]
fn conversion_failure_returns_tuple_array() {
    let (runtime, _) = run(r#"
        let result = Int("abc");
        "#)
    .unwrap();

    let Value::Array(array) = runtime.get_global("result").unwrap() else {
        panic!("expected array result");
    };
    assert!(matches!(array.borrow().values[1], Value::Bool(false)));
}

#[test]
fn reference_examples_compile_and_execute() {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");

    for entry in fs::read_dir(examples_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("crs") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        run(&source).unwrap_or_else(|error| {
            panic!("{} failed: {error}", path.display());
        });
    }
}
