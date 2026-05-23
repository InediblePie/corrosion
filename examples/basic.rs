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
