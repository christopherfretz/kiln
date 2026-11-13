//! Test support for the PCH snapshot corruption fuzzers (WP-1396,
//! `tests/pch_snapshot_fuzz.rs`; spec/snap.md "Threat model"). Hidden:
//! not part of the driver's interface. It knows the PCH file's framing so
//! that the fuzzer does not: where the record's lines, its section block
//! and each section lie, and how to re-frame a PCH after its snapshot
//! section was edited, with every checksum recomputed (the crafted-file
//! model, in which damage reaches the snapshot's decoders).

use kiln_pp::bmi;

use crate::pch::Record;

/// The byte ranges of a PCH file.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// The start of every line of the record's text, then the start of
    /// its section block (the end of the text).
    pub lines: Vec<usize>,
    /// Each section's tag and the file range of its bytes, in file order.
    pub sections: Vec<(u32, std::ops::Range<usize>)>,
    /// The file's length.
    pub len: usize,
}

impl Layout {
    /// Every boundary: line starts, section starts and ends, the end.
    pub fn boundaries(&self) -> Vec<usize> {
        let mut b = self.lines.clone();
        for (_, r) in &self.sections {
            b.push(r.start);
            b.push(r.end);
        }
        b.push(self.len);
        b.sort_unstable();
        b.dedup();
        b
    }

    /// The file range of the snapshot section's bytes.
    pub fn snapshot(&self) -> Option<std::ops::Range<usize>> {
        self.sections.iter().find(|(t, _)| *t == bmi::SECTION_SNAPSHOT).map(|(_, r)| r.clone())
    }
}

/// The layout of the well-formed PCH `pch`.
///
/// # Errors
/// When `pch` is not a readable PCH with a section block.
pub fn layout(pch: &[u8]) -> Result<Layout, String> {
    let rec = Record::parse(pch).map_err(|e| format!("{e:?}"))?;
    let img = rec.image.as_deref().ok_or("no section block")?;
    let text_len = pch.len() - img.len();
    let mut lines = vec![0];
    lines.extend(
        pch[..text_len].iter().enumerate().filter(|(_, b)| **b == b'\n').map(|(i, _)| i + 1),
    );
    let sections = bmi::read_sections(img).map_err(|e| e.to_string())?;
    // Each section's bytes are stored contiguously, in tag order, after
    // their framing: find them in turn.
    let mut out = Vec::new();
    let mut from = text_len;
    for s in &sections {
        let probe = &s.bytes[..s.bytes.len().min(64)];
        let at = if probe.is_empty() {
            from
        } else {
            from + pch[from..]
                .windows(probe.len())
                .position(|w| w == probe)
                .ok_or("section not found")?
        };
        out.push((s.tag, at..at + s.bytes.len()));
        from = at + s.bytes.len();
    }
    Ok(Layout { lines, sections: out, len: pch.len() })
}

/// `pch` with its snapshot section replaced by `edit`'s result and every
/// checksum of the file recomputed.
///
/// # Errors
/// When `pch` is not a readable PCH with a snapshot section.
pub fn edit_snapshot(pch: &[u8], edit: &mut dyn FnMut(&mut Vec<u8>)) -> Result<Vec<u8>, String> {
    let mut rec = Record::parse(pch).map_err(|e| format!("{e:?}"))?;
    let img = rec.image.as_deref().ok_or("no section block")?;
    let mut sections = bmi::read_sections(img).map_err(|e| e.to_string())?;
    let s = sections
        .iter_mut()
        .find(|s| s.tag == bmi::SECTION_SNAPSHOT)
        .ok_or("no snapshot section")?;
    edit(&mut s.bytes);
    rec.image = Some(bmi::write_sections(&sections));
    Ok(rec.write())
}

/// The range, in the snapshot section's bytes `section`, of its encoded
/// lookup log (WP-1403): the head's stream after the schema hash and the
/// identity.
///
/// # Errors
/// When `section` is not a well-formed snapshot section.
pub fn lookup_log_range(section: &[u8]) -> Result<std::ops::Range<usize>, String> {
    let at = crate::pch_snapshot::HEADER_LEN;
    let mut r = kiln_snap::Reader::new(section.get(at..).ok_or("truncated")?);
    let _: u64 = r.get().map_err(|e| e.to_string())?;
    let _: Vec<(String, String)> = r.get().map_err(|e| e.to_string())?;
    let start = at + r.offset();
    let _: kiln_pp::LookupLog = r.get().map_err(|e| e.to_string())?;
    Ok(start..at + r.offset())
}
