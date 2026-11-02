//! The IR-diff normalizations (spec/codegen.md §6, "Comparison modes").
//!
//! Every mode starts from `opt-N -S -passes=verify` output (canonical value
//! and attribute-group numbering) and drops the identification lines
//! ([`base`]). On top of that:
//!
//! - [`structural`] removes what clang/LLVM releases change without
//!   changing the program: comments, metadata, attribute groups and their
//!   `#N` references, `align N` (except a `byval` parameter's), GEP and
//!   arithmetic flags (`inbounds`, `nuw`, `nsw`, `nusw`, `exact`, `nneg`,
//!   `disjoint`, `samesign`, `inrange(..)`), the target datalayout, and
//!   the parameter/return attributes that are not ABI (`noundef`,
//!   `nonnull`, `dereferenceable(N)`, `dead_on_return`, `captures(..)`, ...).
//!   Each run of consecutive comdat (`$x = comdat ..`) or global (`@x = ..`)
//!   lines is sorted: their order means nothing and clang 22 emits a
//!   class's `_ZTI` before its `_ZTS`. Instructions, types, constants,
//!   names, linkage and function order stay.
//! - [`abi_slice`] keeps only what fixes the calling convention and the
//!   record layouts: the `target triple` line, every named type definition, and every `define`/`declare` signature (calling
//!   convention, return and parameter types, the ABI parameter attributes
//!   `byval`/`sret`/`inreg`/`signext`/`zeroext`/`byref`/`inalloca`/
//!   `preallocated`/`nest`/`swift*` with a `byval`'s `align`, mangled
//!   name, varargs) except the `@llvm.*` intrinsic declarations. It is
//!   compared verbatim.

/// Drops the `ModuleID` comment, `source_filename`, the `llvm.ident`
/// named metadata and its string node (clang's or kiln's version).
pub fn base(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        if line.starts_with("; ModuleID")
            || line.starts_with("source_filename")
            || line.starts_with("!llvm.ident")
            || (line.starts_with('!')
                && line.contains(" = !{!\"")
                && (line.contains("clang version") || line.contains("kiln version")))
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Flags on instructions and constant expressions that releases add or
/// infer (LLVM 19 → 22: `nuw` on struct GEPs, `samesign`, `nneg`, ...).
const FLAGS: &[&str] =
    &["inbounds", "nuw", "nsw", "nusw", "exact", "nneg", "disjoint", "samesign"];

/// Parameter, return and function attributes that do not change the ABI.
const NON_ABI_ATTRS: &[&str] = &[
    "noundef",
    "nonnull",
    "noalias",
    "nocapture",
    "dead_on_return",
    "dead_on_unwind",
    "writable",
    "readonly",
    "readnone",
    "writeonly",
    "returned",
    "nofree",
    "noinline",
    "immarg",
    "allocalign",
    "allocptr",
    "local_unnamed_addr",
];

/// Attributes with a parenthesized argument that do not change the ABI.
const NON_ABI_CALL_ATTRS: &[&str] = &[
    "dereferenceable(",
    "dereferenceable_or_null(",
    "captures(",
    "initializes(",
    "range(",
    "nofpclass(",
    "memory(",
    "inrange(",
    "allocsize(",
];

/// Cuts a `;` comment that is outside a quoted string.
fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_str = !in_str,
            ';' if !in_str => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Splits on spaces outside quoted strings.
fn tokens(line: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let mut start = None;
    let mut in_str = false;
    for (i, c) in line.char_indices() {
        if c == '"' {
            in_str = !in_str;
        }
        if c == ' ' && !in_str {
            if let Some(s) = start.take() {
                v.push(&line[s..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        v.push(&line[s..]);
    }
    v
}

/// `tok` without its trailing `,`/`)`/`]` run, and that run.
fn split_punct(tok: &str) -> (&str, &str) {
    let core = tok.trim_end_matches([',', ')', ']']);
    // A balanced `name(..)` keeps its own closing parenthesis.
    let opens = core.matches('(').count();
    let mut cut = core.len();
    let mut closes = core.matches(')').count();
    let bytes = tok.as_bytes();
    while closes < opens && cut < tok.len() && bytes[cut] == b')' {
        cut += 1;
        closes += 1;
    }
    tok.split_at(cut)
}

fn is_dropped_word(core: &str) -> bool {
    FLAGS.contains(&core)
        || NON_ABI_ATTRS.contains(&core)
        || NON_ABI_CALL_ATTRS.iter().any(|p| core.starts_with(p) && core.ends_with(')'))
        || (core.starts_with('#') && core.len() > 1 && core[1..].bytes().all(|b| b.is_ascii_digit()))
}

/// Removes metadata attachments (`, !srcloc !5`, `!llvm.loop !7`): they
/// always trail an instruction.
fn strip_md_attachments(toks: &mut Vec<String>) {
    while toks.len() >= 2 {
        let n = toks.len();
        let name = toks[n - 2].trim_end_matches(',');
        let val = &toks[n - 1];
        if !(val.starts_with('!') && name.starts_with('!') && name.len() > 1) {
            break;
        }
        toks.truncate(n - 2);
        if let Some(last) = toks.last_mut() {
            if last.ends_with(',') {
                last.pop();
            }
        }
    }
}

/// Paren depth change of `tok` (outside quotes).
fn paren_delta(tok: &str) -> i32 {
    tok.chars()
        .map(|c| match c {
            '(' => 1,
            ')' => -1,
            _ => 0,
        })
        .sum()
}

/// One line of [`structural`]; `None` drops it.
fn structural_line(line: &str) -> Option<String> {
    let line = strip_comment(line).trim_end();
    if line.trim().is_empty()
        || line.starts_with('!')
        || line.starts_with("attributes #")
        || line.starts_with("target datalayout")
    {
        return None;
    }
    let indent = &line[..line.len() - line.trim_start().len()];
    let raw = tokens(line.trim_start());
    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut in_byval = false;
    let mut i = 0;
    while i < raw.len() {
        let tok = raw[i];
        // A non-ABI attribute whose argument has spaces (`range(i32 0, 10)`):
        // drop tokens up to the one that balances its parenthesis.
        if NON_ABI_CALL_ATTRS.iter().any(|p| tok.starts_with(p)) {
            let mut depth = paren_delta(tok);
            let mut j = i;
            while depth > 0 && j + 1 < raw.len() {
                j += 1;
                depth += paren_delta(raw[j]);
            }
            // The closing token's surplus `)` belong to the enclosing list:
            // drop the attribute's own closing parentheses from its trailing
            // punctuation run and keep the rest.
            let last = raw[j];
            let surplus = usize::try_from(-depth.min(0)).unwrap_or(0);
            let trailing = &last[last.trim_end_matches([',', ')', ']']).len()..];
            let mut own = trailing.matches(')').count().saturating_sub(surplus);
            let kept: String = trailing
                .chars()
                .filter(|&c| {
                    if c == ')' && own > 0 {
                        own -= 1;
                        return false;
                    }
                    true
                })
                .collect();
            attach(&mut out, &kept);
            if kept.contains(',') || kept.contains(')') {
                in_byval = false;
            }
            i = j + 1;
            continue;
        }
        let (core, punct) = split_punct(tok);
        if core.starts_with("byval(") {
            in_byval = true;
        }
        if core == "align" && i + 1 < raw.len() {
            let (num, punct2) = split_punct(raw[i + 1]);
            if !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()) {
                if in_byval {
                    out.push(tok.to_owned());
                    out.push(raw[i + 1].to_owned());
                } else {
                    attach(&mut out, punct2);
                }
                if !punct2.is_empty() {
                    in_byval = false;
                }
                i += 2;
                continue;
            }
        }
        if is_dropped_word(core) {
            attach(&mut out, punct);
        } else {
            out.push(tok.to_owned());
        }
        if punct.contains(',') || punct.contains(')') {
            in_byval = false;
        }
        i += 1;
    }
    strip_md_attachments(&mut out);
    let mut s = out.join(" ");
    // A dropped `, align N` leaves its comma on the previous token.
    while s.ends_with(',') {
        s.pop();
    }
    Some(format!("{indent}{s}"))
}

/// Appends a dropped token's trailing punctuation to the previous token.
fn attach(out: &mut Vec<String>, punct: &str) {
    if punct.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if !(last.ends_with(',') && punct.starts_with(',')) => last.push_str(punct),
        Some(last) => last.push_str(&punct[1..]),
        None => out.push(punct.to_owned()),
    }
}

/// The structural form of an `opt -S` text (module docs).
pub fn structural(s: &str) -> String {
    let lines: Vec<String> = base(s).lines().filter_map(structural_line).collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let lead = lines[i].as_bytes()[0];
        let mut j = i + 1;
        if matches!(lead, b'$' | b'@') {
            while j < lines.len() && lines[j].as_bytes()[0] == lead {
                j += 1;
            }
        }
        let mut run: Vec<&str> = lines[i..j].iter().map(String::as_str).collect();
        run.sort_unstable();
        for l in run {
            out.push_str(l);
            out.push('\n');
        }
        i = j;
    }
    out
}

/// Linkage, visibility and other header words that are not the ABI.
const NOT_SIGNATURE: &[&str] = &[
    "private",
    "internal",
    "available_externally",
    "linkonce",
    "weak",
    "common",
    "appending",
    "extern_weak",
    "linkonce_odr",
    "weak_odr",
    "external",
    "dso_local",
    "dso_preemptable",
    "hidden",
    "protected",
    "default",
    "dllimport",
    "dllexport",
    "unnamed_addr",
];

/// Splits `params` (without the outer parentheses) at top-level commas.
fn split_params(params: &str) -> Vec<&str> {
    let mut v = Vec::new();
    let (mut depth, mut start, mut in_str) = (0i32, 0, false);
    for (i, c) in params.char_indices() {
        match c {
            '"' => in_str = !in_str,
            '(' | '{' | '[' | '<' if !in_str => depth += 1,
            ')' | '}' | ']' | '>' if !in_str => depth -= 1,
            ',' if depth == 0 && !in_str => {
                v.push(params[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = params[start..].trim();
    if !last.is_empty() {
        v.push(last);
    }
    v
}

/// The ABI signature of a structural `define`/`declare` line.
fn signature(line: &str) -> Option<String> {
    let at = line.find(" @")?;
    let open = at + line[at..].find('(')?;
    let mut depth = 0i32;
    let mut close = None;
    for (i, c) in line[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let head: Vec<&str> =
        line[..at].split(' ').filter(|w| !w.is_empty() && !NOT_SIGNATURE.contains(w)).collect();
    let params: Vec<String> = split_params(&line[open + 1..close])
        .into_iter()
        .map(|p| {
            let mut t = tokens(p);
            if t.len() >= 2 && t.last().is_some_and(|l| l.starts_with('%')) {
                t.pop();
            }
            t.join(" ")
        })
        .collect();
    Some(format!("{} {}({})", head.join(" "), &line[at + 1..open], params.join(", ")))
}

/// The ABI slice of an `opt -S` text (module docs).
pub fn abi_slice(s: &str) -> String {
    let mut out = String::new();
    for line in base(s).lines() {
        let line = strip_comment(line).trim_end();
        let kept = if line.starts_with("target triple") {
            Some(line.to_owned())
        } else if line.starts_with('%') && line.contains(" = type ") {
            Some(line.to_owned())
        } else if line.starts_with("declare ") && line.contains(" @llvm.") {
            // Intrinsics are not calls through the ABI.
            None
        } else if line.starts_with("define ") || line.starts_with("declare ") {
            structural_line(line).and_then(|l| signature(&l))
        } else {
            None
        };
        if let Some(k) = kept {
            out.push_str(&k);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_strips_release_noise() {
        let a = "define dso_local noundef i32 @_Z1fP1S(ptr noundef %0) #0 {\n  \
                 %2 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 1\n  \
                 %3 = load i32, ptr %2, align 4, !tbaa !5\n  ret i32 %3\n}\n\
                 attributes #0 = { noinline }\n!5 = !{}\n";
        let b = "define dso_local i32 @_Z1fP1S(ptr %0) #1 {\n  \
                 %2 = getelementptr %struct.S, ptr %0, i32 0, i32 1\n  \
                 %3 = load i32, ptr %2\n  ret i32 %3\n}\n";
        assert_eq!(structural(a), structural(b));
        assert!(structural(a).contains("load i32, ptr %2\n"), "{}", structural(a));
    }

    #[test]
    fn structural_sorts_comdat_and_global_runs() {
        let a = "$_ZTS1A = comdat any\n$_ZTI1A = comdat any\n\n@_ZTS1A = constant [3 x i8] c\"1A\\00\"\n@_ZTI1A = constant { ptr } { ptr @_ZTS1A }\n";
        let b = "$_ZTI1A = comdat any\n$_ZTS1A = comdat any\n\n@_ZTI1A = constant { ptr } { ptr @_ZTS1A }\n@_ZTS1A = constant [3 x i8] c\"1A\\00\"\n";
        assert_eq!(structural(a), structural(b));
        assert!(structural(a).starts_with("$_ZTI1A"), "{}", structural(a));
    }

    #[test]
    fn structural_keeps_byval_align_and_abi_attrs() {
        let s = "declare void @g(ptr dead_on_unwind writable sret(%struct.S) align 8, ptr noundef byval(%struct.T) align 16, i8 noundef signext) #0\n";
        assert_eq!(
            structural(s),
            "declare void @g(ptr sret(%struct.S), ptr byval(%struct.T) align 16, i8 signext)\n"
        );
    }

    #[test]
    fn abi_slice_is_the_signatures_and_types() {
        let s = "target triple = \"x86_64-unknown-linux-gnu\"\n\
                 %struct.S = type { i32, [4 x i8] }\n\
                 @g = global i32 0, align 4\n\
                 define linkonce_odr dso_local noundef { i64, i64 } @_Z1fv(i32 noundef %a, ...) #0 comdat {\n  ret void\n}\n";
        assert_eq!(
            abi_slice(s),
            "target triple = \"x86_64-unknown-linux-gnu\"\n\
             %struct.S = type { i32, [4 x i8] }\n\
             define { i64, i64 } @_Z1fv(i32, ...)\n"
        );
    }

    #[test]
    fn comments_outside_strings_only() {
        assert_eq!(strip_comment("x = c\"a;b\" ; c"), "x = c\"a;b\" ");
        assert_eq!(structural("5:   ; preds = %4\n"), "5:\n");
    }
}
