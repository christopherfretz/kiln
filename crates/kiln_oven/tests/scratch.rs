mod common;
use common::*;
use kiln_oven::{Budget, Oven};

#[test]
fn dump() {
    let src = std::fs::read_to_string("../../target/scratch/cases.txt").unwrap();
    let mut out = String::new();
    for c in src.split("\n====\n") {
        let r = std::panic::catch_unwind(|| {
            let mut tu = Tu::new(c);
            let mut oven = Oven::new(Budget::default());
            match tu.run(&mut oven) {
                Ok(()) => "OK\n".to_string(),
                Err(f) => f.text,
            }
        });
        out.push_str(&r.unwrap_or_else(|_| "PANIC\n".into()));
        out.push_str("====\n");
    }
    std::fs::write("../../target/scratch/out.txt", out).unwrap();
}
