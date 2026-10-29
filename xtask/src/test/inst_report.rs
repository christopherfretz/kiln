//! The `inst-report:` directive's report handling: the NDJSON template
//! instantiation report kiln prints on stderr under
//! `-fkiln-instantiation-report=json` (`spec/sema-templates.md` §12),
//! extracted from the compile's stderr and with its wall-clock timings
//! masked so a golden is stable across runs and hosts.

/// The report lines of `stderr` (one `{"template"…}` row per template,
/// then the `{"total"…}` row), masked, each newline-terminated.
pub fn extract(stderr: &str) -> String {
    let mut out = String::new();
    for line in stderr.lines() {
        let l = line.trim_end();
        if l.starts_with("{\"template\"") || l.starts_with("{\"total\"") {
            out.push_str(&mask_line(l));
            out.push('\n');
        }
    }
    out
}

/// `report` (a golden) with every line's timings masked.
pub fn mask(report: &str) -> String {
    let mut out = String::new();
    for line in report.lines() {
        let l = line.trim_end();
        if l.is_empty() {
            continue;
        }
        out.push_str(&mask_line(l));
        out.push('\n');
    }
    out
}

/// Replaces the value of every `"<name>_us":<digits>` field with `0`.
fn mask_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("_us\":") {
        let (head, tail) = rest.split_at(i + "_us\":".len());
        out.push_str(head);
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            out.push('0');
        }
        rest = &tail[digits..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_report_rows_and_masks_timings() {
        let stderr = "a.cpp:1:1: warning: x\n\
            {\"template\":\"A<int>\",\"kind\":\"class\",\"count\":1,\"self_us\":88,\"incl_us\":120}\n\
            1 warning generated.\n\
            {\"total\":true,\"count\":1,\"self_us\":88}\n";
        assert_eq!(
            extract(stderr),
            "{\"template\":\"A<int>\",\"kind\":\"class\",\"count\":1,\"self_us\":0,\"incl_us\":0}\n\
             {\"total\":true,\"count\":1,\"self_us\":0}\n"
        );
    }

    #[test]
    fn no_report_is_empty() {
        assert_eq!(extract("error: x\n1 error generated.\n"), "");
    }

    #[test]
    fn golden_mask_is_idempotent_and_skips_blank_lines() {
        let g = "{\"total\":true,\"self_us\":0}\n\n";
        assert_eq!(mask(g), "{\"total\":true,\"self_us\":0}\n");
        assert_eq!(mask(&mask(g)), mask(g));
    }

    #[test]
    fn mask_leaves_other_fields_alone() {
        assert_eq!(
            mask_line("{\"bytes\":226,\"name_us\":\"x\"}"),
            "{\"bytes\":226,\"name_us\":\"x\"}"
        );
    }
}
