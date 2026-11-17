//! WP-1586: mortar's O-13 corpus of kind 2 hash tests, as files
//! (`tests/mason/corpus/<case>/`, spec/harness.md "The O-13 corpus"):
//! every case is compiled and its expectation checked through the
//! decoder. `scripts/mason/odr_corpus.py` builds the same objects and their
//! `manifest.tsv` for a linker's test lane; both read `case.tsv`.

mod mason_common;

use std::path::{Path, PathBuf};

use mason_common::*;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mason/corpus").canonicalize().unwrap()
}

/// One unit's (layout, size, align) of the kind 3 row of type `name`.
fn type_row(b: &[u8], name: &str) -> Option<(u64, u32, u16)> {
    let at = block_at(b, 3)?;
    let (row, rows) = (u32_at(b, at + 8) as usize, u32_at(b, at + 12) as usize);
    (0..rows)
        .map(|i| at + 16 + i * row)
        .find(|&r| u64_at(b, r) == fnv(name))
        .map(|r| (u64_at(b, r + 24), u32_at(b, r + 36), u16_at(b, r + 40)))
}

/// Builds one case's objects in `out`; returns each object's container.
fn build(src: &Path, out: &Path, units: &[&str]) -> Vec<Vec<u8>> {
    let dir = src.to_str().unwrap();
    units
        .iter()
        .map(|u| {
            let w: Vec<&str> = u.split(' ').collect();
            let (file, std) = (w[0], format!("-std={}", w[1]));
            let flags: Vec<String> = w[2..].iter().map(|f| f.replace("{dir}", dir)).collect();
            let stem = file.trim_end_matches(".cpp");
            let mut a: Vec<String> = vec![std.clone()];
            for f in &flags {
                if let Some(h) = f.strip_prefix("@pch:") {
                    let pch = out.join(format!("{stem}.pch")).to_str().unwrap().to_owned();
                    let mut p: Vec<&str> = vec![&std];
                    p.extend(flags.iter().filter(|f| !f.starts_with('@')).map(String::as_str));
                    p.extend(["-x", "c++-header", h, "-o", &pch]);
                    ok(src, &p);
                    a.extend(["-include-pch".into(), pch]);
                } else {
                    a.push(f.clone());
                }
            }
            let obj = out.join(format!("{stem}.o")).to_str().unwrap().to_owned();
            a.extend([
                "-fmason-metadata".into(),
                "-c".into(),
                file.into(),
                "-o".into(),
                obj.clone(),
            ]);
            ok(src, &a.iter().map(String::as_str).collect::<Vec<_>>());
            container(&std::fs::read(&obj).unwrap())
        })
        .collect()
}

/// The failures of one case.
fn check(case: &str, line: &str, out: &Path) -> Vec<String> {
    let f: Vec<&str> = line.trim_end_matches('\n').split('\t').collect();
    assert!(f.len() >= 7, "{case}: case.tsv: {line:?}");
    let (expect, symbols, reason) = (f[0], f[1], f[4]);
    assert!(["equal", "differ", "excluded"].contains(&expect), "{case}: {expect}");
    assert!(["min", "default", "detail", "all"].contains(&f[3]), "{case}: level {}", f[3]);
    assert!((expect == "excluded") != (reason == "-"), "{case}: a reason exactly when excluded");
    let d = out.join(case);
    std::fs::create_dir_all(&d).unwrap();
    let objs = build(&corpus_dir().join(case), &d, &f[5..]);
    let mut bad = Vec::new();
    for s in symbols.split(' ') {
        if let Some(t) = s.strip_prefix("type:") {
            let got: Vec<_> = objs.iter().map(|b| type_row(b, t)).collect();
            assert!(got.iter().all(Option::is_some), "{case}: {s}: no kind 3 row: {got:?}");
            if got.windows(2).all(|w| w[0] == w[1]) == (expect == "differ") {
                bad.push(format!("{case} ({expect}): {s}: {got:?}"));
            }
            continue;
        }
        let rows: Vec<Option<Def>> =
            objs.iter().map(|b| def_rows(b).and_then(|r| row(&r, s).cloned())).collect();
        if reason == "X15" {
            if rows.iter().any(Option::is_some) {
                bad.push(format!("{case}: {s}: a row (X15: none)"));
            }
            continue;
        }
        let Some(rows) = rows.into_iter().collect::<Option<Vec<Def>>>() else {
            bad.push(format!("{case}: {s}: a unit has no row"));
            continue;
        };
        let one_hash = rows.windows(2).all(|w| w[0].hash == w[1].hash);
        let k13 = rows.iter().any(|r| r.flags & EXPLICIT_SPEC != 0)
            && rows.iter().any(|r| r.flags & IMPLICIT_INST != 0);
        let loose = |r: &Def| r.flags & !(VAGUE | IMPLICIT_INST | EXPLICIT_INST);
        let ok = match expect {
            "equal" => one_hash && !k13 && rows.windows(2).all(|w| loose(&w[0]) == loose(&w[1])),
            "differ" => !one_hash || k13 || rows.iter().any(|r| r.flags & USES_INTERNAL != 0),
            _ => one_hash,
        };
        if !ok {
            bad.push(format!("{case} ({expect}): {s}: {rows:x?}"));
        }
    }
    bad
}

#[test]
fn o13_corpus() {
    let out = scratch("corpus");
    let mut cases: Vec<String> = std::fs::read_dir(corpus_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    cases.sort();
    let mut bad = Vec::new();
    let mut n = [0; 3];
    for c in &cases {
        let line = std::fs::read_to_string(corpus_dir().join(c).join("case.tsv")).unwrap();
        n[["equal", "differ", "excluded"].iter().position(|e| line.starts_with(e)).unwrap()] += 1;
        bad.extend(check(c, &line, &out));
    }
    assert!(bad.is_empty(), "corpus failures:\n{}", bad.join("\n"));
    assert!(n[0] >= 20 && n[1] >= 20 && n[2] >= 3, "{n:?}");
}
