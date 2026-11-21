//! Unit tests of the release gate (WP-1219; spec/harness.md "Release
//! gate"): the manifest, resumption, the record, `check`, `import-x86`.

use std::fs;
use std::path::{Path, PathBuf};

use super::manifest::{self, Host, Kind, Measured};
use super::record::{self, Ctx, Status, StepRecord};
use super::{Gate, Opts};

const C1: &str = "1111111111111111111111111111111111111111";
const C2: &str = "2222222222222222222222222222222222222222";

fn projects() -> Vec<String> {
    vec!["fmt".to_owned(), "llvm_full".to_owned()]
}

fn ctx() -> Ctx {
    Ctx { commit: C1.into(), version: "1.3.0".into(), smoke: false, kiln: Some("k1".into()) }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-release-gate-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Every step has a minimum unit count and a pass rule; every OSS row and
/// the LLVM rows are steps; ids are unique; dependencies come first.
#[test]
fn manifest_is_complete() {
    let steps = manifest::steps(&projects(), &[("downstream".into(), "true".into())]);
    let mut seen = Vec::new();
    for s in &steps {
        assert!(s.min_units > 0 && s.smoke_min_units > 0, "{}", s.id);
        assert!(!s.rule.is_empty() && !s.counts.is_empty(), "{}", s.id);
        assert!(s.deps.iter().all(|d| seen.contains(d)), "{} before its deps", s.id);
        assert!(!seen.contains(&s.id), "{} twice", s.id);
        seen.push(s.id.clone());
    }
    for id in [
        "archive",
        "merge-gate",
        "oracle19",
        "pch-sweep",
        "torture-gnu23",
        "torture-gnu17",
        "libcxx-aarch64",
        "libcxx-x86_64",
        "gxx-dg-aarch64",
        "gxx-dg-x86_64",
        "oss-x86_64",
        "oss-fmt",
        "oss-llvm_full",
        "external-downstream",
    ] {
        assert!(seen.iter().any(|s| s == id), "{id}");
    }
    let toml = "# c\n[fmt]\nwave = 1\nsweep = true\n\n[llvm_full]\nwave = 4\n";
    assert_eq!(manifest::projects_of(toml), projects());
    assert_eq!(super::sweep_projects(toml), vec!["fmt".to_owned()]);
    // The real table: every row of scripts/oss/projects.toml.
    let real =
        fs::read_to_string(crate::util::repo_root().join("scripts/oss/projects.toml")).unwrap();
    let rows = manifest::projects_of(&real);
    assert!(rows.len() > 50 && rows.iter().any(|p| p == "llvm_selfhost"), "{rows:?}");
}

fn done(step: &str, commit: &str, counts: bool) -> StepRecord {
    StepRecord {
        step: step.into(),
        commit: commit.into(),
        version: "1.3.0".into(),
        mode: "release".into(),
        kiln: "k1".into(),
        start: 1_763_400_000,
        end: 1_763_403_600,
        load: "3.1-4.0".into(),
        arch: "aarch64".into(),
        counts: if counts { vec![("pass".into(), 5)] } else { Vec::new() },
        units: counts.then_some(5),
        pass: true,
        detail: "ok".into(),
        log: "/dev/null".into(),
        log_hash: "h".into(),
    }
}

/// A state directory with some steps done, one stale by commit, one by
/// missing counts: `plan` and `run --dry-run` see it the same way.
#[test]
fn resumption_and_stale_rule() {
    let dir = scratch("resume");
    let steps = manifest::steps(&projects(), &[]);
    let write =
        |r: StepRecord| fs::write(dir.join(format!("{}.rec", r.step)), r.to_text()).unwrap();
    write(done("torture-gnu23", C1, true));
    write(done("torture-gnu17", C2, true));
    write(done("gxx-dg-aarch64", C1, false));
    let mut kiln_other = done("oss-fmt", C1, true);
    kiln_other.kiln = "k0".into();
    write(kiln_other);
    let g = Gate {
        root: crate::util::repo_root(),
        state: dir.clone(),
        ctx: ctx(),
        steps,
        kiln: Some(PathBuf::from("/bin/true")),
        baseline: Vec::new(),
        pgo: true,
    };
    // The records' logs are /dev/null: hash of nothing.
    for id in ["torture-gnu23", "torture-gnu17", "gxx-dg-aarch64", "oss-fmt"] {
        let p = dir.join(format!("{id}.rec"));
        let t = fs::read_to_string(&p)
            .unwrap()
            .replace("log_hash: h", &format!("log_hash: {}", record::fnv64(b"")));
        fs::write(&p, t).unwrap();
    }
    let st = super::statuses(&g);
    let by = |id: &str| st[g.steps.iter().position(|s| s.id == id).unwrap()].clone();
    assert!(matches!(by("torture-gnu23"), Status::Done(_)));
    assert!(matches!(by("torture-gnu17"), Status::Stale(w) if w.contains("commit 222222222")));
    assert!(matches!(by("gxx-dg-aarch64"), Status::Stale(w) if w.contains("no counts")));
    assert!(matches!(by("oss-fmt"), Status::Stale(w) if w.contains("kiln++ k0")));
    assert_eq!(by("archive"), Status::Missing);
    // A record made in a smoke gate never counts in a release gate.
    let mut smoke = done("torture-gnu23", C1, true);
    smoke.mode = "smoke".into();
    assert!(
        matches!(record::status(&g.steps[5], Some(smoke), &g.ctx, Some("h")), Status::Stale(w) if w.contains("smoke"))
    );
    // `run --dry-run --step ...` names what it would do, runs nothing.
    let o = Opts {
        cmd: "run".into(),
        dry_run: true,
        smoke: true,
        steps: vec!["torture-gnu23".into(), "torture-gnu17".into()],
        ..Opts::default()
    };
    let sel: Vec<&str> = super::selected(&g, &o).unwrap().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(sel, ["torture-gnu23", "torture-gnu17"]);
    assert!(super::run(&g, &o).unwrap());
    assert!(!dir.join("logs").exists(), "a dry run writes nothing");
    let o = Opts { steps: vec!["no-such".into()], ..o };
    assert!(super::selected(&g, &o).is_err());
    let _ = fs::remove_dir_all(&dir);
}

fn complete(steps: &[manifest::Step]) -> Vec<Status> {
    steps.iter().map(|s| Status::Done(done(&s.id, C1, true))).collect()
}

/// `record` renders every section from complete state and fails naming
/// the steps without a completed record; `check` accepts the result and
/// rejects a hand edit, a row at another commit and a smoke record.
#[test]
fn record_and_check() {
    let steps = manifest::steps(&projects(), &[("downstream".into(), "x".into())]);
    let mut st = complete(&steps);
    let secs =
        record::render(&steps, &st, &ctx(), "# c\ng++.dg/a.C [-std=c++26] row WP-1\n").unwrap();
    let text = record::splice("", "kiln 1.3.0 release measurements", &secs, &ctx());
    for id in [
        "`archive`",
        "`oss-llvm_full`",
        "`external-downstream`",
        "row WP-1",
        "**Status: every step passed.**",
    ] {
        assert!(text.contains(id), "{id}");
    }
    assert!(record::check(&text, &steps).is_ok(), "{:?}", record::check(&text, &steps));
    // Hand-written text outside the markers survives a second splice.
    let edited = text.replace("## Summary\n", "## Summary\n\nWritten by hand.\n");
    let again = record::splice(&edited, "t", &secs, &ctx());
    assert!(again.contains("Written by hand.") && record::check(&again, &steps).is_ok());
    // A hand edit inside a region.
    let bad = text.replacen("| pass |", "| pass  |", 1);
    assert!(record::check(&bad, &steps).unwrap_err().iter().any(|e| e.contains("edited by hand")));
    // A row at another commit (and the hash fixed up to hide the edit).
    let mut c2 = ctx();
    c2.commit = C2.into();
    st[3] = Status::Done(done(&steps[3].id, C2, true));
    let mixed = record::render(&steps, &st, &ctx(), "").unwrap();
    let t2 = record::splice("", "t", &mixed, &ctx());
    let errs = record::check(&t2, &steps).unwrap_err();
    assert!(errs.iter().any(|e| e.contains("row at commit 222222222")), "{errs:?}");
    // Missing steps: `record` fails and names each.
    st[0] = Status::Missing;
    st[5] = Status::Stale("made at commit 222222222".into());
    let e = record::render(&steps, &st, &ctx(), "").unwrap_err();
    assert!(
        e.contains("2 manifest steps")
            && e.contains("archive (no record)")
            && e.contains("torture-gnu23 (stale"),
        "{e}"
    );
    // A missing row; a smoke record.
    let fewer: Vec<_> = steps.iter().filter(|s| s.id != "oss-fmt").cloned().collect();
    let t3 = record::splice(
        "",
        "t",
        &record::render(&fewer, &complete(&fewer), &ctx(), "").unwrap(),
        &ctx(),
    );
    assert!(record::check(&t3, &steps).unwrap_err().iter().any(|e| e == "step oss-fmt: no row"));
    let smoke = Ctx { smoke: true, ..ctx() };
    let t4 = record::splice(
        "",
        "t",
        &record::render(&steps, &complete(&steps), &smoke, "").unwrap(),
        &smoke,
    );
    assert!(t4.contains("SMOKE GATE: NOT A RELEASE RECORD"));
    assert!(record::check(&t4, &steps).unwrap_err().iter().any(|e| e.contains("mode smoke")));
}

/// The readers: ci lines, the pch verdict, case TSVs, libc++ results,
/// OSS reports and their oss.md baseline; the minimum unit count.
#[test]
fn readers_and_rules() {
    let ci = "xtask ci: corpus (kiln): test result: ok. 6947 passed; 0 failed; 31 xfail; 7096 tests in 1.0s\n\
              xtask ci: corpus (kiln --bmi-kind 1): test result: ok. 150 passed; 0 failed; 151 tests in 1s\n";
    let m = manifest::read_ci(ci, true);
    assert!(m.pass && m.units == 6947, "{m:?}");
    let failed = ci.replace("ok. 150 passed; 0 failed", "FAILED. 149 passed; 1 failed");
    assert!(!manifest::read_ci(&failed, false).pass);
    assert!(!manifest::read_ci("", true).pass, "no corpus line: not a pass");
    let p = manifest::read_pch(
        "libc++ pair: 4299 tests, 0 differences\npch-release-gate: pass\n",
        true,
    );
    assert!(p.pass && p.units == 4299);
    assert!(!manifest::read_pch("pch-release-gate: FAIL\n", false).pass);
    let tsv = "test\topt\tkiln\tkiln_detail\nexecute/a.c\t-O0\tpass\t\nexecute/a.c\t-O2\twrong\tx\nexecute/b.c\t-O0\tskip\tw\n";
    assert_eq!(manifest::read_case_tsv(tsv), (2, 1, 0));
    let lx = manifest::read_libcxx(&[
        "# h\nPASS\ta\t\t\t\nFAIL\tb\td\tk\t\n".into(),
        "UNSUPPORTED\tc\t\t\t\n".into(),
    ]);
    assert_eq!((lx.count("pass"), lx.count("fail"), lx.units), (Some(1), Some(1), 3));
    let mut lx2 = lx.clone();
    manifest::not_below(
        &mut lx2,
        "libcxx-x86_64",
        &manifest::read_baseline("# c\nlibcxx-x86_64\tpass\t2\tr\n"),
    );
    assert!(!lx2.pass && lx2.detail.contains("pass 1 < baseline 2"));
    // lesson (a): an OSS row with 0 of 0 TUs and exit 0 fails.
    let mut empty = manifest::read_oss_report(r#"{"tus": {"ok": 0, "total": 0}}"#).unwrap();
    manifest::enforce_min(&mut empty, 1);
    assert!(!empty.pass && empty.detail.starts_with("ran 0 units"));
    let md = "<!-- oss:glaze -->\n| | kiln | clang-19 |\n|---|---|---|\n| TUs ok / failed | 87 / 2 | 87 / 2 |\n\
              | tests passed / total | 100 / 102 | 100 / 102 |\n<!-- /oss:glaze -->\n";
    assert_eq!(manifest::oss_baseline(md, "glaze"), Some((Some(87), Some(100))));
    let mut r = manifest::read_oss_report(
        r#"{"tus": {"ok": 87, "total": 89}, "tests": {"passed": 99, "total": 102}}"#,
    )
    .unwrap();
    manifest::oss_verdict(&mut r, manifest::oss_baseline(md, "glaze"));
    assert!(!r.pass && r.detail.contains("tests_passed 99 < baseline 100"));
    // WP-1718: a hosted row's baseline is capped at the same run's clang
    // control (opencv builds fewer TUs on x86-64 than on aarch64).
    let clang = manifest::read_oss_report(
        r#"{"tus": {"ok": 80, "total": 80}, "tests": {"passed": 100, "total": 102}}"#,
    )
    .unwrap();
    let capped = manifest::oss_baseline_capped(manifest::oss_baseline(md, "glaze"), Some(&clang));
    assert_eq!(capped, Some((Some(80), Some(100))));
    let mut x86 = manifest::read_oss_report(
        r#"{"tus": {"ok": 80, "total": 80}, "tests": {"passed": 100, "total": 102}}"#,
    )
    .unwrap();
    manifest::oss_verdict(&mut x86, capped);
    assert!(x86.pass, "{}", x86.detail);
    let mut low = manifest::read_oss_report(
        r#"{"tus": {"ok": 6, "total": 80}, "tests": {"passed": 11, "total": 102}}"#,
    )
    .unwrap();
    manifest::oss_verdict(&mut low, capped);
    assert!(!low.pass && low.detail.contains("tus_ok 6 < baseline 80"), "{}", low.detail);
    assert_eq!(
        manifest::oss_baseline_capped(manifest::oss_baseline(md, "glaze"), None),
        Some((Some(87), Some(100)))
    );
    assert_eq!(record::utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(record::utc(1_763_424_000), "2025-11-18 00:00 UTC");
    let rec = done("x", C1, true);
    assert_eq!(StepRecord::parse(&rec.to_text()), rec);
    assert!(matches!(Measured::default().counts_line().as_str(), ""));
}

/// `import-x86` on a fixture laid out like the sweep's downloaded
/// artifacts (`gh run download`): one directory per artifact, each with
/// the `commit` file sweep.yml writes. Rows are made up.
#[test]
fn import_x86_fixture() {
    let dir = scratch("import");
    let art = dir.join("artifacts");
    let put = |p: &str, t: &str| {
        let f = art.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, t).unwrap();
    };
    put("kiln-dist/kiln-1.3.0-x86_64-unknown-linux-gnu.tar.xz", "no commit file: not read");
    put("kiln-build/commit", &format!("{C1}\n"));
    put("kiln-build/build-info.txt", &build_info(C1, true).to_text());
    for a in [
        "libcxx-1",
        "libcxx-2",
        "libcxx-summary",
        "gxx-dg-1",
        "gxx-dg-summary",
        "oss-fmt-kiln",
        "oss-fmt-clang",
    ] {
        put(&format!("{a}/commit"), &format!("{C1}\n"));
    }
    put("libcxx-1/libcxx-1.tsv", "PASS\tstd/a.pass.cpp\t\t\t\n");
    put("libcxx-2/libcxx-2.tsv", "PASS\tstd/b.pass.cpp\t\t\t\nUNSUPPORTED\tstd/c.pass.cpp\t\t\t\n");
    put("gxx-dg-1/gxx-dg-1.tsv", "test\topt\tkiln\n");
    let gxx = "test\topt\tkiln\tkiln_detail\tgcc\tgcc_detail\tclang\tclang_detail\tnegative\txfail\tdropped\n\
               g++.dg/x.C\t-std=c++26\tpass\t\t\t\t\t\t\t\t\n";
    put("gxx-dg-summary/gxx-dg-x86_64.tsv", gxx);
    put(
        "oss-fmt-kiln/report.json",
        r#"{"tus": {"ok": 3, "total": 3}, "tests": {"passed": 2, "total": 2}}"#,
    );
    let state = dir.join("state");
    fs::create_dir_all(state.join("logs")).unwrap();
    let g = Gate {
        root: crate::util::repo_root(),
        state: state.clone(),
        ctx: Ctx { smoke: true, ..ctx() },
        steps: manifest::steps(&projects(), &[]),
        kiln: None,
        baseline: Vec::new(),
        pgo: true,
    };
    // All rows pass the smoke minimums; the gxx compare runs the real script.
    let ok = super::import_x86(&g, art.to_str().unwrap()).unwrap();
    let rec =
        |id: &str| StepRecord::parse(&fs::read_to_string(state.join(format!("{id}.rec"))).unwrap());
    assert_eq!(rec("libcxx-x86_64").units, Some(3));
    assert_eq!(rec("libcxx-x86_64").commit, C1);
    assert_eq!(rec("gxx-dg-x86_64").units, Some(1));
    let oss = rec("oss-x86_64");
    assert_eq!(oss.counts.iter().find(|c| c.0 == "reported").map(|c| c.1), Some(1));
    assert!(oss.detail.contains("kiln++: PGO build, profile sha256 0123456789abcdef"), "{oss:?}");
    // A release import of a plain hosted build records nothing; with
    // --no-pgo it is accepted.
    put("kiln-build/build-info.txt", &build_info(C1, false).to_text());
    let release = Gate { ctx: ctx(), ..g.clone() };
    let e = super::import_x86(&release, art.to_str().unwrap()).unwrap_err();
    assert!(e.contains("not what ships: plain build (no PGO), but the release"), "{e}");
    put("kiln-build/build-info.txt", &build_info(C1, true).to_text());
    assert!(ok || !rec("gxx-dg-x86_64").pass, "only the one-row g++.dg run may fail its compare");
    // An artifact of another commit: nothing is imported.
    put("oss-fmt-kiln/commit", C2);
    let e = super::import_x86(&g, art.to_str().unwrap()).unwrap_err();
    assert!(e.contains("oss-fmt-kiln is from 222222222"), "{e}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hosted_steps_need_no_kiln() {
    let steps = manifest::steps(&projects(), &[]);
    for s in &steps {
        if s.host == Host::Hosted {
            assert!(!s.uses_kiln() && s.scratch_gb == 0, "{}", s.id);
        }
        if matches!(s.kind, Kind::Oss { project: Some(_) }) {
            assert_eq!(s.deps, ["archive"]);
        }
    }
    assert!(Path::new(&crate::util::repo_root().join("docs/status/gxx-dg-baseline.txt")).is_file());
}

/// Every step gets a driver named `kiln++`: a symlink is not resolved to
/// `kiln` (which would link as C, without the C++ runtime), a `kiln` maps
/// to its sibling `kiln++`, and any other name is refused.
#[test]
fn compiler_is_named_kilnpp() {
    let d = scratch("driver");
    fs::write(d.join("kiln"), "bin").unwrap();
    std::os::unix::fs::symlink("kiln", d.join("kiln++")).unwrap();
    fs::write(d.join("kiln-dev"), "bin").unwrap();
    let want = d.join("kiln++");
    assert_eq!(super::cxx_driver(&d.join("kiln++")).unwrap(), want);
    assert_eq!(super::cxx_driver(&d.join("kiln")).unwrap(), want);
    let e = super::cxx_driver(&d.join("kiln-dev")).unwrap_err();
    assert!(e.contains("must be named `kiln++`"), "{e}");
    let lone = scratch("driver-lone");
    fs::write(lone.join("kiln"), "bin").unwrap();
    assert!(super::cxx_driver(&lone.join("kiln")).unwrap_err().contains("no such kiln++"));
    let _ = fs::remove_dir_all(&d);
    let _ = fs::remove_dir_all(&lone);
}

fn build_info(commit: &str, pgo: bool) -> crate::dist::BuildInfo {
    crate::dist::BuildInfo {
        version: "1.3.0".into(),
        triple: "x86_64-unknown-linux-gnu".into(),
        commit: commit.into(),
        pgo: pgo.then(|| crate::dist::PgoInfo {
            profile_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            profile_bytes: 1234,
            llvm_profdata: "22.1.8".into(),
            training_cases: 44,
            unprofiled_kiln: 0,
            unprofiled_host_code: 1187,
        }),
    }
}

/// WP-1862: `dist`'s build-info.txt round-trips, and the gate's verdict on
/// it: a PGO build of this commit whose profile applied passes; a plain
/// build passes only with `--no-pgo`; another commit, a profile that left
/// kiln's functions without data, or no file fail. The archive row's
/// detail reaches the record's summary.
#[test]
fn pgo_build_verdict() {
    use crate::dist::BuildInfo;
    let pgo = build_info(C1, true);
    assert_eq!(BuildInfo::parse(&pgo.to_text()), Some(pgo.clone()));
    let plain = build_info(C1, false);
    assert_eq!(BuildInfo::parse(&plain.to_text()), Some(plain.clone()));
    assert_eq!(BuildInfo::parse("version: 1.3.0\n"), None, "no pgo: line");
    assert_eq!(BuildInfo::parse("pgo: yes\n"), None, "pgo without a profile");
    let v = |i: Option<&BuildInfo>, pgo: bool| super::build_verdict(i, C1, pgo);
    let (ok, what) = v(Some(&pgo), true);
    assert!(ok, "{what}");
    assert_eq!(
        what,
        "PGO build, profile sha256 0123456789abcdef (44 training cases, llvm-profdata 22.1.8; \
         0 of kiln's functions without profile data)"
    );
    assert!(v(Some(&pgo), false).0, "--no-pgo accepts a PGO build too");
    assert_eq!(v(Some(&plain), false), (true, "plain build (no PGO), --no-pgo".to_owned()));
    let (ok, what) = v(Some(&plain), true);
    assert!(!ok && what.contains("but the release archives are PGO-built"), "{what}");
    let (ok, what) = v(Some(&build_info(C2, true)), true);
    assert!(!ok && what.ends_with("but built from 222222222"), "{what}");
    let mut cold = pgo.clone();
    if let Some(p) = cold.pgo.as_mut() {
        p.unprofiled_kiln = super::UNPROFILED_MAX + 1;
    }
    let (ok, what) = v(Some(&cold), true);
    assert!(!ok && what.contains("the profile did not apply"), "{what}");
    assert!(!v(None, true).0);

    let steps = manifest::steps(&projects(), &[("downstream".into(), "x".into())]);
    let mut st = complete(&steps);
    let mut a = done("archive", C1, true);
    a.detail =
        format!("kiln version 1.3.0; kiln++ k1; {}", super::build_verdict(Some(&pgo), C1, true).1);
    st[0] = Status::Done(a);
    let secs = record::render(&steps, &st, &ctx(), "").unwrap();
    let sum = &secs.iter().find(|(s, _)| *s == "summary").unwrap().1;
    assert!(sum.contains("is the release archive's: kiln version 1.3.0; kiln++ k1; PGO build, profile sha256 0123456789abcdef"), "{sum}");
    // The external gets the archive's kiln++ and is tied to its hash.
    let ext = steps.iter().find(|s| s.id == "external-downstream").unwrap();
    assert!(ext.uses_kiln() && ext.deps == ["archive"]);
}
