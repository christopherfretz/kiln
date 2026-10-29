//! Splitting `-fdump-record-layouts` and `-fdump-vtable-layouts` output
//! into comparable blocks (spec/abi.md §6, §8.4, §8.5).
//!
//! kiln's dumps are compared with clang 19's by the driver differentials
//! (`kiln_driver/tests/{record,vtable}_layouts.rs`) and by the test
//! harness's `expect: layout` and `expect: vtables` modes
//! (`spec/harness.md` "Layout and vtable tests"). Both use these
//! functions, so the rules are written once:
//!
//! - **Record layouts** ([`record_layout_blocks`]) are compared block by
//!   block, in order. clang's builtin records, which it lays out before
//!   any user class and kiln never dumps, are dropped.
//! - **Vtables** ([`vtable_blocks`]) are compared as a *set* of blocks.
//!   clang dumps in emission order and may dump a construction vtable more
//!   than once; kiln dumps each group once in definition order. clang's
//!   `Original map` sections (its cache of overridden methods, not part
//!   of any layout) are dropped.
//!
//! These are text utilities only: they do not lay anything out.

use std::collections::BTreeSet;

/// The line that starts each record-layout block (with the newline
/// before it).
pub const RECORD_LAYOUT_MARKER: &str = "\n*** Dumping AST Record Layout\n";

/// Records clang lays out for its own builtins (`__NSConstantString`,
/// `va_list` on each target); a block whose class line names one is not
/// compared.
pub const CLANG_BUILTIN_RECORDS: [&str; 3] =
    ["__NSConstantString_tag", "__va_list_tag", "std::__va_list"];

/// The first lines of the blocks of a vtable dump.
pub const VTABLE_BLOCK_HEADS: [&str; 5] = [
    "Vtable for ",
    "Construction vtable for ",
    "VTable indices for ",
    "Thunks for ",
    "Virtual base offset offsets for ",
];

/// The record-layout blocks of `text`, in order, each without the marker
/// (it starts with the class line), with clang's builtin records dropped.
/// Text before the first marker is ignored.
pub fn record_layout_blocks(text: &str) -> Vec<&str> {
    text.split(RECORD_LAYOUT_MARKER)
        .skip(1)
        .filter(|b| {
            let head = b.lines().next().unwrap_or_default();
            !CLANG_BUILTIN_RECORDS.iter().any(|n| head.contains(n))
        })
        .collect()
}

/// The set of vtable-dump blocks of `text`: each starts at a line with one
/// of [`VTABLE_BLOCK_HEADS`] and runs to the next. `Original map`
/// sections (the line and the indented lines after it) are dropped, and
/// so are leading and trailing blank lines of each block; the lines are
/// joined with `\n`, without a final newline. Text before the first head
/// forms a block of its own (normally there is none).
pub fn vtable_blocks(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut in_map = false;
    let mut flush = |cur: &mut Vec<&str>| {
        while cur.last().is_some_and(|l| l.is_empty()) {
            cur.pop();
        }
        if !cur.is_empty() {
            out.insert(cur.join("\n"));
        }
        cur.clear();
    };
    for line in text.lines() {
        if line == "Original map" {
            in_map = true;
            continue;
        }
        if in_map {
            if line.starts_with(' ') {
                continue;
            }
            in_map = false;
        }
        if VTABLE_BLOCK_HEADS.iter().any(|h| line.starts_with(h)) {
            flush(&mut cur);
        }
        if !cur.is_empty() || !line.is_empty() {
            cur.push(line);
        }
    }
    flush(&mut cur);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_layout_blocks_drop_builtins() {
        let text = "\n*** Dumping AST Record Layout\n         0 | struct std::__va_list\n\
                    \n*** Dumping AST Record Layout\n         0 | struct E (empty)\n\
                    \n*** Dumping AST Record Layout\n         0 | struct A\n";
        assert_eq!(
            record_layout_blocks(text),
            ["         0 | struct E (empty)\n", "         0 | struct A\n"]
        );
        assert_eq!(record_layout_blocks(""), Vec::<&str>::new());
        assert_eq!(record_layout_blocks("no marker\n"), Vec::<&str>::new());
    }

    #[test]
    fn vtable_blocks_are_a_set_without_original_maps() {
        let clang = "Original map\n void L::g() -> void V::g()\n\
                     Vtable for 'L' (1 entries).\n   0 | L RTTI\n\n\
                     Thunks for 'void L::g()' (1 entry).\n   0 | x\n\n\
                     Original map\n\
                     Construction vtable for ('L', 0) in 'D' (1 entries).\n   0 | L RTTI\n\n\
                     Construction vtable for ('L', 0) in 'D' (1 entries).\n   0 | L RTTI\n\n";
        let kiln = "Construction vtable for ('L', 0) in 'D' (1 entries).\n   0 | L RTTI\n\n\
                    Vtable for 'L' (1 entries).\n   0 | L RTTI\n\n\
                    Thunks for 'void L::g()' (1 entry).\n   0 | x\n";
        let b = vtable_blocks(clang);
        assert_eq!(b.len(), 3, "{b:#?}");
        assert!(b.contains("Vtable for 'L' (1 entries).\n   0 | L RTTI"));
        assert_eq!(b, vtable_blocks(kiln));
        assert!(vtable_blocks("\n\n").is_empty());
    }
}
