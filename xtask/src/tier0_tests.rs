//! The tier-0 workflow's containment contract (WP-1902; spec/harness.md
//! "CI", docs/VALIDATION.md "Tier-0 (outside pull requests)"):
//! `.github/workflows/tier0.yml` runs untrusted code from outside pull
//! requests, so it triggers only on `pull_request`, holds a read-only token,
//! references no secret, times every job, skips same-repo pull requests and
//! cancels only its own pull request's older run. Each rule is checked on
//! the real file, and a mutated copy must break exactly that rule.

use crate::util;

/// The workflow the outside-PR pipeline approves and reads, by path.
const TIER0: &str = ".github/workflows/tier0.yml";

/// The `if:` every job carries: same-repo pull requests skip.
const SAME_REPO_SKIP: &str =
    "if: github.event.pull_request.head.repo.full_name != github.repository";

/// The per-pull-request concurrency group.
const GROUP: &str = "group: tier0-${{ github.event.pull_request.number }}";

/// Whether a line is not a whole-line comment.
fn is_code(l: &str) -> bool {
    !l.trim_start().starts_with('#')
}

/// The lines of the top-level map `key:` (its block, up to the next
/// top-level key), code only; `None` when the key is absent.
fn top_block<'a>(yml: &'a str, key: &str) -> Option<Vec<&'a str>> {
    let head = format!("{key}:");
    let mut lines = yml.lines();
    lines.find(|l| l.split_once(" #").map_or(*l, |(c, _)| c).trim_end() == head)?;
    Some(
        lines
            .take_while(|l| l.is_empty() || l.starts_with([' ', '#']))
            .filter(|l| !l.is_empty() && is_code(l))
            .collect(),
    )
}

/// The jobs: each two-space-indented key under `jobs:` and its lines.
fn jobs(yml: &str) -> Vec<(String, Vec<&str>)> {
    let mut out: Vec<(String, Vec<&str>)> = Vec::new();
    for l in top_block(yml, "jobs").unwrap_or_default() {
        if l.len() > 2 && l.starts_with("  ") && !l[2..].starts_with(' ') {
            out.push((l.trim().trim_end_matches(':').to_owned(), Vec::new()));
        } else if let Some((_, b)) = out.last_mut() {
            b.push(l);
        }
    }
    out
}

/// Every rule `yml` breaks, as one line each (empty: contained).
fn problems(yml: &str) -> Vec<String> {
    let mut p = Vec::new();
    let code: Vec<&str> = yml.lines().filter(|l| is_code(l)).collect();

    // 1. The only trigger is `pull_request`, and nothing names the
    //    privileged triggers.
    match top_block(yml, "on") {
        Some(on) => {
            let triggers: Vec<&str> = on
                .iter()
                .filter(|l| l.len() > 2 && !l[2..].starts_with(' '))
                .map(|l| l.trim().trim_end_matches(':'))
                .collect();
            if triggers != ["pull_request"] {
                p.push(format!("triggers are {triggers:?}, not only pull_request"));
            }
        }
        None => p.push("no block `on:` map (write `on:` then `  pull_request:`)".to_owned()),
    }
    for t in ["pull_request_target", "workflow_run"] {
        if code.iter().any(|l| l.contains(t)) {
            p.push(format!("names {t}"));
        }
    }

    // 2. A read-only token: top-level `permissions: contents: read` only,
    //    and no job widens it.
    match top_block(yml, "permissions") {
        Some(perm) if perm.iter().map(|l| l.trim()).eq(["contents: read"]) => {}
        Some(perm) => p.push(format!("top-level permissions are {perm:?}, not contents: read")),
        None => p.push("no top-level `permissions:` map".to_owned()),
    }

    // 3. Every job has a timeout; 5. and the same-repo skip; no job
    //    overrides the permissions.
    let jobs = jobs(yml);
    if jobs.is_empty() {
        p.push("no jobs".to_owned());
    }
    for (name, b) in &jobs {
        let key = |k: &str| b.iter().find_map(|l| l.strip_prefix(&format!("    {k}: ")));
        match key("timeout-minutes").map(|t| t.trim().parse::<u32>()) {
            Some(Ok(1..=60)) => {}
            Some(_) => p.push(format!("job `{name}`: timeout-minutes is not 1 to 60")),
            None => p.push(format!("job `{name}` has no timeout-minutes")),
        }
        if !b.iter().any(|l| l.trim() == SAME_REPO_SKIP) {
            p.push(format!("job `{name}` lacks `{SAME_REPO_SKIP}`"));
        }
        if b.iter().any(|l| l.trim_start().starts_with("permissions:")) {
            p.push(format!("job `{name}` sets its own permissions"));
        }
    }

    // 4. No secret, and no artifact for a later workflow.
    for needle in ["secrets.", "secrets:", "upload-artifact"] {
        if code.iter().any(|l| l.contains(needle)) {
            p.push(format!("references `{needle}`"));
        }
    }

    // 6. Concurrency per pull request, never one shared group.
    let conc = top_block(yml, "concurrency").unwrap_or_default();
    let conc: Vec<&str> = conc.iter().map(|l| l.trim()).collect();
    if conc != [GROUP, "cancel-in-progress: true"] {
        p.push(format!("concurrency is {conc:?}, not `{GROUP}` with cancel-in-progress"));
    }
    p
}

fn tier0() -> String {
    std::fs::read_to_string(util::repo_root().join(TIER0)).unwrap()
}

#[test]
fn tier0_workflow_is_contained() {
    let yml = tier0();
    assert_eq!(problems(&yml), Vec::<String>::new(), "{TIER0}");
    // The full CI still runs on every pull request (tier-0 is no substitute).
    let ci = std::fs::read_to_string(util::repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("\n  pull_request:\n"), "ci.yml no longer runs on pull_request");
}

/// Each mutation breaks one rule, and `problems` names that rule.
#[test]
fn tier0_mutations_are_caught() {
    let yml = tier0();
    let replace = |from: &str, to: &str| {
        assert!(yml.contains(from), "{TIER0} lacks `{from}`; update the mutation");
        yml.replacen(from, to, 1)
    };
    let first_timeout = yml
        .lines()
        .find(|l| l.starts_with("    timeout-minutes: "))
        .expect("a job timeout")
        .to_owned();
    let mutations: Vec<(&str, String, &str)> = vec![
        (
            "a push trigger",
            replace("on:\n  pull_request:\n", "on:\n  pull_request:\n  push:\n"),
            "triggers are",
        ),
        (
            "pull_request_target",
            replace("on:\n  pull_request:\n", "on:\n  pull_request_target:\n"),
            "triggers are",
        ),
        (
            "an inline on:",
            replace("on:\n  pull_request:\n", "on: [pull_request, push]\n"),
            "no block `on:`",
        ),
        (
            "write permissions",
            replace("permissions:\n  contents: read\n", "permissions:\n  contents: write\n"),
            "top-level permissions",
        ),
        (
            "an extra permission",
            replace("  contents: read\n", "  contents: read\n  pull-requests: write\n"),
            "top-level permissions",
        ),
        (
            "no permissions",
            replace("permissions:\n  contents: read\n", ""),
            "no top-level `permissions:`",
        ),
        (
            "job permissions",
            replace(
                &format!("{first_timeout}\n"),
                &format!("{first_timeout}\n    permissions: write-all\n"),
            ),
            "sets its own permissions",
        ),
        ("a missing timeout", replace(&format!("{first_timeout}\n"), ""), "has no timeout-minutes"),
        (
            "a secret",
            replace(
                "      - run: cargo xtask ci",
                "      - env: { T: \"${{ secrets.TOKEN }}\" }\n        run: cargo xtask ci",
            ),
            "references `secrets.`",
        ),
        (
            "an upload",
            replace(
                "      - run: cargo xtask ci",
                "      - uses: actions/upload-artifact@v4\n      - run: cargo xtask ci",
            ),
            "upload-artifact",
        ),
        ("no same-repo skip", replace(&format!("    {SAME_REPO_SKIP}\n"), ""), "lacks `if:"),
        ("one shared group", replace(GROUP, "group: tier0"), "concurrency is"),
    ];
    for (what, mutated, want) in mutations {
        assert_ne!(mutated, yml, "{what}: the mutation changed nothing");
        let got = problems(&mutated);
        assert!(got.iter().any(|g| g.contains(want)), "{what}: expected `{want}`, got {got:?}");
    }
}
