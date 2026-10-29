//! The constexpr report (`-fconstexpr-report=<path>`): parsing and budget
//! checks. The format is normative in `spec/oven.md` "Reporting protocol";
//! `cargo xtask test` checks `oven:` budgets against it and `cargo xtask
//! bench` records its total line.

use std::fmt::Write as _;

use super::header::{Counter, OvenBudget};
use super::json::Json;

/// The flag that asks kiln for a report.
pub const FLAG: &str = "-fconstexpr-report=";

/// What a top-level evaluation was for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalKind {
    StaticAssert,
    Init,
    TemplateArg,
    Consteval,
    IfConstexpr,
    Constraint,
    Other,
}

impl EvalKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "static_assert" => Self::StaticAssert,
            "init" => Self::Init,
            "template_arg" => Self::TemplateArg,
            "consteval" => Self::Consteval,
            "if_constexpr" => Self::IfConstexpr,
            "constraint" => Self::Constraint,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

/// Counters of one evaluation, or of several combined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub steps: u64,
    pub bytes: u64,
    pub peak_bytes: u64,
    pub calls: u64,
    pub allocs: u64,
    pub peak_arena: u64,
    pub frozen_bytes: u64,
    pub wall_us: u64,
    /// How many evaluations these counters cover.
    pub evaluations: u64,
}

impl Counters {
    pub fn get(&self, c: Counter) -> u64 {
        match c {
            Counter::Steps => self.steps,
            Counter::Bytes => self.bytes,
            Counter::PeakBytes => self.peak_bytes,
            Counter::Calls => self.calls,
            Counter::Allocs => self.allocs,
            Counter::PeakArena => self.peak_arena,
            Counter::FrozenBytes => self.frozen_bytes,
            Counter::Evaluations => self.evaluations,
        }
    }

    /// Combines evaluations: every counter is summed except `peak_bytes`
    /// and `peak_arena`, which are maxima (each top-level evaluation starts
    /// from empty frame and heap spaces, so the peaks do not add up).
    /// Saturates rather than overflows.
    pub fn combine(&self, o: &Self) -> Self {
        Self {
            steps: self.steps.saturating_add(o.steps),
            bytes: self.bytes.saturating_add(o.bytes),
            peak_bytes: self.peak_bytes.max(o.peak_bytes),
            calls: self.calls.saturating_add(o.calls),
            allocs: self.allocs.saturating_add(o.allocs),
            peak_arena: self.peak_arena.max(o.peak_arena),
            frozen_bytes: self.frozen_bytes.saturating_add(o.frozen_bytes),
            wall_us: self.wall_us.saturating_add(o.wall_us),
            evaluations: self.evaluations.saturating_add(o.evaluations),
        }
    }

    /// `steps=1 bytes=2 ...`, the counters a budget can bound.
    pub fn describe(&self) -> String {
        Counter::ALL
            .iter()
            .map(|c| format!("{}={}", c.name(), self.get(*c)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// `file:line:col` of an evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loc {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

impl Loc {
    /// The file may itself contain `:`; line and column are the last two
    /// fields and are 1-based.
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.rsplitn(3, ':');
        let col = it.next()?.parse().ok().filter(|&c| c > 0)?;
        let line = it.next()?.parse().ok().filter(|&l| l > 0)?;
        let file = it.next().filter(|f| !f.is_empty())?;
        Some(Self { file: file.to_owned(), line, col })
    }
}

/// One top-level evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evaluation {
    /// `None` for an evaluation with no source location (`"loc": null`).
    pub loc: Option<Loc>,
    pub kind: EvalKind,
    /// `evaluations` is always 1.
    pub counters: Counters,
}

/// A parsed, cross-checked report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub evaluations: Vec<Evaluation>,
    /// The final `{"total": ...}` line.
    pub total: Counters,
}

impl Report {
    /// Parses and validates a report. Errors name the 1-based line.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut evaluations = Vec::new();
        let mut total = None;
        for (idx, raw) in text.lines().enumerate() {
            let n = idx + 1;
            if raw.trim().is_empty() {
                continue;
            }
            let at = |e: String| format!("line {n}: {e}");
            if total.is_some() {
                return Err(at("text after the `total` line".into()));
            }
            let j = Json::parse(raw).map_err(at)?;
            if j.as_obj().is_none() {
                return Err(at("expected a JSON object".into()));
            }
            if let Some(t) = j.get("total") {
                let c = counters(t, true).map_err(|e| at(format!("in `total`: {e}")))?;
                total = Some(c);
            } else {
                evaluations.push(evaluation(&j).map_err(at)?);
            }
        }
        let total = total.ok_or("no `total` line (it must be the last line)")?;
        let sum = evaluations.iter().fold(Counters::default(), |acc, e| acc.combine(&e.counters));
        let mut bad = Vec::new();
        for c in Counter::ALL {
            if total.get(c) != sum.get(c) {
                let rule = if matches!(c, Counter::PeakBytes | Counter::PeakArena) {
                    "maximum"
                } else if c == Counter::Evaluations {
                    "count"
                } else {
                    "sum"
                };
                bad.push(format!(
                    "`{}` is {} but the {rule} over the evaluations is {}",
                    c.name(),
                    total.get(c),
                    sum.get(c)
                ));
            }
        }
        if !bad.is_empty() {
            return Err(format!(
                "the total line disagrees with the evaluations: {}",
                bad.join("; ")
            ));
        }
        Ok(Self { evaluations, total })
    }

    /// The combined counters of the evaluations at `line` (and `col`, if
    /// given) of a file accepted by `in_file`.
    pub fn at(&self, line: u32, col: Option<u32>, in_file: &dyn Fn(&str) -> bool) -> Counters {
        self.evaluations
            .iter()
            .filter(|e| {
                e.loc.as_ref().is_some_and(|l| {
                    l.line == line && col.is_none_or(|c| c == l.col) && in_file(&l.file)
                })
            })
            .fold(Counters::default(), |acc, e| acc.combine(&e.counters))
    }
}

fn evaluation(j: &Json) -> Result<Evaluation, String> {
    let loc = match j.get("loc").ok_or("missing `loc`")? {
        Json::Null => None,
        Json::Str(s) => {
            Some(Loc::parse(s).ok_or_else(|| format!("bad `loc` {s:?} (file:line:col)"))?)
        }
        _ => return Err("`loc` must be a string or null".into()),
    };
    let kind = j.get("kind").ok_or("missing `kind`")?;
    let kind = kind.as_str().and_then(EvalKind::parse).ok_or(
        "`kind` must be one of static_assert|init|template_arg|consteval|if_constexpr|constraint|other",
    )?;
    let mut counters = counters(j, false)?;
    counters.evaluations = 1;
    Ok(Evaluation { loc, kind, counters })
}

/// The counter fields of an evaluation line, or of the total object (which
/// also has `evaluations`).
fn counters(j: &Json, total: bool) -> Result<Counters, String> {
    if j.as_obj().is_none() {
        return Err("expected an object".into());
    }
    let n = |k: &str| -> Result<u64, String> {
        j.get(k)
            .ok_or_else(|| format!("missing `{k}`"))?
            .as_u64()
            .ok_or_else(|| format!("`{k}` must be a non-negative integer (at most 2^53)"))
    };
    Ok(Counters {
        steps: n("steps")?,
        bytes: n("bytes")?,
        peak_bytes: n("peak_bytes")?,
        calls: n("calls")?,
        allocs: n("allocs")?,
        peak_arena: n("peak_arena")?,
        frozen_bytes: n("frozen_bytes")?,
        wall_us: n("wall_us")?,
        evaluations: if total { n("evaluations")? } else { 0 },
    })
}

/// Checks `budgets` against `report`. A located budget applies to the
/// evaluations at its location in a file accepted by `in_test_file`, and at
/// least one must be there. `Err` lists every breach.
pub fn check_budgets(
    report: &Report,
    budgets: &[OvenBudget],
    in_test_file: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let mut out = String::new();
    for b in budgets {
        let (what, got) = match b.at {
            None => ("total".to_owned(), report.total),
            Some(at) => {
                let c = report.at(at.line, at.col, in_test_file);
                if c.evaluations == 0 {
                    let _ = writeln!(
                        out,
                        "oven: no evaluation reported at {at} of the test file (budget {})",
                        describe_limits(b)
                    );
                    continue;
                }
                (format!("at {at}"), c)
            }
        };
        for &(c, limit) in &b.limits {
            if got.get(c) > limit {
                let _ = writeln!(
                    out,
                    "oven: {what}: {} is {}, over the budget of {limit}",
                    c.name(),
                    got.get(c)
                );
            }
        }
    }
    if out.is_empty() { Ok(()) } else { Err(out) }
}

fn describe_limits(b: &OvenBudget) -> String {
    b.limits.iter().map(|(c, n)| format!("{}<={n}", c.name())).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::header::parse_oven;

    const GOOD: &str = concat!(
        r#"{"loc":"t.cpp:3:1","kind":"static_assert","steps":10,"bytes":64,"peak_bytes":32,"calls":2,"allocs":1,"peak_arena":40,"frozen_bytes":16,"wall_us":5}"#,
        "\n",
        r#"{"loc":"t.cpp:3:15","kind":"init","steps":5,"bytes":0,"peak_bytes":0,"calls":1,"allocs":0,"peak_arena":0,"frozen_bytes":24,"wall_us":1,"extra":"ignored"}"#,
        "\n",
        r#"{"loc":"C:\\dir\\h.h:3:1","kind":"template_arg","steps":1,"bytes":8,"peak_bytes":8,"calls":0,"allocs":1,"peak_arena":8,"frozen_bytes":0,"wall_us":0}"#,
        "\n",
        r#"{"loc":null,"kind":"constraint","steps":100,"bytes":100,"peak_bytes":50,"calls":7,"allocs":2,"peak_arena":64,"frozen_bytes":100,"wall_us":9}"#,
        "\n\n",
        r#"{"total":{"steps":116,"bytes":172,"peak_bytes":50,"calls":10,"allocs":4,"peak_arena":64,"frozen_bytes":140,"wall_us":17,"evaluations":4}}"#,
        "\n",
    );

    fn in_t(f: &str) -> bool {
        f == "t.cpp"
    }

    #[test]
    fn parses_a_report() {
        let r = Report::parse(GOOD).unwrap();
        assert_eq!(r.evaluations.len(), 4);
        assert_eq!(r.evaluations[0].loc, Some(Loc { file: "t.cpp".into(), line: 3, col: 1 }));
        assert_eq!(r.evaluations[0].kind, EvalKind::StaticAssert);
        assert_eq!(r.evaluations[2].loc.as_ref().unwrap().file, r"C:\dir\h.h");
        assert_eq!(r.evaluations[3].loc, None);
        assert_eq!(r.evaluations[3].kind, EvalKind::Constraint);
        assert_eq!(r.total.steps, 116);
        assert_eq!(r.total.evaluations, 4);
        assert_eq!(
            r.total.describe(),
            "steps=116 bytes=172 peak_bytes=50 calls=10 allocs=4 peak_arena=64 frozen_bytes=140 \
             evaluations=4"
        );
        // Located aggregation: line 3 of t.cpp only (not h.h, not the null loc).
        let at3 = r.at(3, None, &in_t);
        assert_eq!((at3.steps, at3.peak_bytes, at3.evaluations), (15, 32, 2));
        assert_eq!((at3.peak_arena, at3.frozen_bytes), (40, 40));
        assert_eq!(r.evaluations[2].kind, EvalKind::TemplateArg);
        let at3_15 = r.at(3, Some(15), &in_t);
        assert_eq!((at3_15.steps, at3_15.evaluations), (5, 1));
        assert_eq!(r.at(4, None, &in_t).evaluations, 0);
        // An empty report is just the total line.
        let empty = Report::parse(
            "{\"total\":{\"steps\":0,\"bytes\":0,\"peak_bytes\":0,\"calls\":0,\"allocs\":0,\"peak_arena\":0,\"frozen_bytes\":0,\"wall_us\":3,\"evaluations\":0}}\n",
        )
        .unwrap();
        assert_eq!(empty.evaluations.len(), 0);
    }

    #[test]
    fn rejects_malformed_reports() {
        let cases: &[(&str, &str)] = &[
            ("", "no `total` line"),
            ("not json\n", "line 1:"),
            ("[1]\n", "expected a JSON object"),
            (
                r#"{"kind":"init","steps":1,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}"#,
                "missing `loc`",
            ),
            (
                r#"{"loc":"t.cpp:1","kind":"init","steps":1,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}"#,
                "bad `loc`",
            ),
            (
                r#"{"loc":null,"kind":"lambda","steps":1,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}"#,
                "`kind` must be one of",
            ),
            (
                r#"{"loc":null,"kind":"init","steps":-1,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}"#,
                "`steps` must be a non-negative integer",
            ),
            (
                r#"{"loc":null,"kind":"init","steps":1.5,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}"#,
                "`steps` must be",
            ),
            (
                r#"{"loc":null,"kind":"init","steps":1,"bytes":0,"peak_bytes":0,"calls":0,"wall_us":0}"#,
                "missing `allocs`",
            ),
            (
                r#"{"loc":null,"kind":"init","steps":1,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"frozen_bytes":0,"wall_us":0}"#,
                "missing `peak_arena`",
            ),
            (r#"{"total":{"steps":0}}"#, "in `total`: missing `bytes`"),
            (
                r#"{"total":{"steps":0,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0}}"#,
                "missing `evaluations`",
            ),
        ];
        for (text, want) in cases {
            let e = Report::parse(text).expect_err(text);
            assert!(e.contains(want), "{text}\n  got: {e}\n  want: {want}");
        }
        // Text after the total line, and a second total line.
        let total = r#"{"total":{"steps":0,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0,"evaluations":0}}"#;
        let e = Report::parse(&format!("{total}\n{total}\n")).unwrap_err();
        assert!(e.contains("line 2: text after the `total` line"), "{e}");
        // Totals that disagree with the evaluations.
        let bad = GOOD
            .replace(r#""steps":116"#, r#""steps":117"#)
            .replace(r#""peak_bytes":50,"calls":10"#, r#""peak_bytes":90,"calls":10"#);
        let e = Report::parse(&bad).unwrap_err();
        assert!(e.contains("`steps` is 117 but the sum over the evaluations is 116"), "{e}");
        assert!(e.contains("`peak_bytes` is 90 but the maximum over the evaluations is 50"), "{e}");
        let bad = GOOD.replace(
            r#""peak_arena":64,"frozen_bytes":140"#,
            r#""peak_arena":65,"frozen_bytes":139"#,
        );
        let e = Report::parse(&bad).unwrap_err();
        assert!(e.contains("`peak_arena` is 65 but the maximum over the evaluations is 64"), "{e}");
        assert!(e.contains("`frozen_bytes` is 139 but the sum over the evaluations is 140"), "{e}");
        let bad = GOOD.replace(r#""evaluations":4"#, r#""evaluations":5"#);
        assert!(Report::parse(&bad).unwrap_err().contains("count over the evaluations is 4"));
        // Duplicate keys are malformed JSON for this reader.
        let dup = r#"{"total":{"steps":0,"steps":0,"bytes":0,"peak_bytes":0,"calls":0,"allocs":0,"peak_arena":0,"frozen_bytes":0,"wall_us":0,"evaluations":0}}"#;
        assert!(Report::parse(dup).is_err());
    }

    #[test]
    fn checks_budgets() {
        let r = Report::parse(GOOD).unwrap();
        let b = |s: &str| parse_oven(s).unwrap();
        assert_eq!(check_budgets(&r, &[b("steps<=116 evaluations<=4")], &in_t), Ok(()));
        assert_eq!(check_budgets(&r, &[b("3 steps<=15 peak_bytes<=32")], &in_t), Ok(()));
        let e = check_budgets(
            &r,
            &[b("steps<=115 peak_bytes<=49"), b("3:15 calls<=0"), b("9 steps<=1")],
            &in_t,
        )
        .unwrap_err();
        assert_eq!(
            e,
            "oven: total: steps is 116, over the budget of 115\n\
             oven: total: peak_bytes is 50, over the budget of 49\n\
             oven: at 3:15: calls is 1, over the budget of 0\n\
             oven: no evaluation reported at 9 of the test file (budget steps<=1)\n"
        );
    }
}
