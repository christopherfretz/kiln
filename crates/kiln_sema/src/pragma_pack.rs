//! `#pragma pack`, `#pragma options align` and `#pragma align`
//! ([cpp.pragma]/1; clang `Sema::ActOnPragmaPack`,
//! `Sema::ActOnPragmaOptionsAlign` and `AlignPackStack`;
//! spec/sema-class.md §6.1).
//!
//! The three pragmas share one stack. The value in effect where a class
//! definition begins caps its members' alignment (`record_pack`, applied
//! by `act_on_start_record_definition`). The forms clang ignores are
//! ignored with clang's warnings; `pack(show)` prints the value. kiln's
//! targets are Linux and Windows, where `mac68k` alignment is clang's
//! error ("not supported on this target", `hasAlignMac68kSupport` is
//! Darwin-only), and `power` is `native` (only AIX differs).

use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// What a `#pragma pack(...)` does (clang `PragmaMsStackAction`): no flag
/// is `pack()`, a reset to the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[allow(clippy::struct_excessive_bools, reason = "clang's four independent action bits")]
pub struct PackAction {
    /// `PSK_Set`: an alignment is given.
    pub set: bool,
    /// `PSK_Push`.
    pub push: bool,
    /// `PSK_Pop`.
    pub pop: bool,
    /// `PSK_Show`.
    pub show: bool,
}

/// The alignment of a `#pragma pack`, as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PackAlignArg {
    /// An integer literal's value (`None`: too large for 64 bits).
    Int(Option<u64>),
    /// A numeric literal that is not an integer (`pack(1.0)`).
    NotInteger,
}

/// The argument of `#pragma options align=k` / `#pragma align=k` (clang
/// `PragmaOptionsAlignKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionsAlign {
    /// `native`.
    Native,
    /// `natural`.
    Natural,
    /// `packed`.
    Packed,
    /// `power` (AIX's; `native` elsewhere).
    Power,
    /// `mac68k` (Darwin only).
    Mac68k,
    /// `reset`.
    Reset,
}

/// The alignment mode of a stack value (clang `AlignPackInfo::Mode`;
/// `Mac68k` never gets in, see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) enum AlignMode {
    #[default]
    Native,
    Natural,
    Packed,
}

/// One value of the stack (clang `AlignPackInfo` on kiln's targets).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct PackValue {
    mode: AlignMode,
    /// Set by `#pragma pack` (clang `IsPackAttr`).
    pack_attr: bool,
    /// The pack value in bytes (0: none).
    n: u16,
}

impl PackValue {
    /// The value whose [`PackValue::max_field_align`] is `n` (a class
    /// template pattern's, for its instantiations).
    pub(crate) fn with_max_field_align(n: u16) -> Self {
        PackValue { mode: AlignMode::Native, pack_attr: n != 0, n }
    }

    /// The maximum member alignment it imposes in bytes (0: none; clang
    /// `AddAlignmentAttributesForRecord`'s `MaxFieldAlignmentAttr`).
    pub(crate) fn max_field_align(self) -> u16 {
        if self.pack_attr { self.n } else { u16::from(self.mode == AlignMode::Packed) }
    }
}

/// A pushed value (clang `PragmaStack::Slot`).
#[derive(Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct PackSlot {
    label: Option<String>,
    value: PackValue,
    /// The pragma that set `value` (none: the default).
    value_loc: Option<Loc>,
    /// The pushing pragma.
    push_loc: Loc,
}

/// The stack and the current value (clang `AlignPackStack`).
#[derive(Clone, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct PackStack {
    pub(crate) current: PackValue,
    /// The pragma that set `current` (clang `CurrentPragmaLocation`).
    current_loc: Option<Loc>,
    stack: Vec<PackSlot>,
}

impl PackStack {
    /// clang `PragmaStack::Act` for the pragma at `loc`.
    fn act(&mut self, loc: Loc, action: PackAction, label: Option<&str>, value: PackValue) {
        if action == PackAction::default() {
            self.current = PackValue::default();
            self.current_loc = Some(loc);
            return;
        }
        if action.push {
            self.stack.push(PackSlot {
                label: label.map(str::to_owned),
                value: self.current,
                value_loc: self.current_loc,
                push_loc: loc,
            });
        } else if action.pop {
            let i = match label {
                Some(l) => self.stack.iter().rposition(|s| s.label.as_deref() == Some(l)),
                None => self.stack.len().checked_sub(1),
            };
            if let Some(i) = i {
                self.current = self.stack[i].value;
                self.current_loc = self.stack[i].value_loc;
                self.stack.truncate(i);
            }
        }
        if action.set {
            self.current = value;
            self.current_loc = Some(loc);
        }
    }
}

impl Sema<'_, '_> {
    /// `#pragma pack(...)` at `loc` (its `pack` token), parsed by the
    /// parser (clang `PragmaPackHandler`) into `action`, the label and the
    /// alignment; clang's `ActOnPragmaPack`.
    pub fn act_on_pragma_pack(
        &mut self,
        loc: Loc,
        action: PackAction,
        label: Option<&str>,
        align: Option<PackAlignArg>,
    ) {
        let mode = self.pack.current.mode;
        let mut n = 0u16;
        if let Some(a) = align {
            // pack(0) is pack() for the value; any other value must be
            // a power of two up to 16.
            match a {
                PackAlignArg::Int(Some(v)) if v == 0 || (v.is_power_of_two() && v <= 16) => {
                    n = u16::try_from(v).unwrap_or(0);
                }
                _ => {
                    let d = self.diag(DiagId::sema_pragma_pack_invalid_alignment, loc);
                    self.emit(d);
                    return;
                }
            }
        }
        if action.show {
            let shown = match self.pack.current.max_field_align() {
                0 => 8,
                v => v,
            };
            let d = self.diag(DiagId::sema_pragma_pack_show, loc).arg(u32::from(shown));
            self.emit(d);
        }
        if action.pop {
            if align.is_some() && label.is_some() {
                let d = self.diag(DiagId::sema_pragma_pack_pop_identifier_and_alignment, loc);
                self.emit(d);
            }
            if self.pack.stack.is_empty() {
                let d =
                    self.diag(DiagId::sema_pragma_pop_failed, loc).arg("pack").arg("stack empty");
                self.emit(d);
            }
        }
        self.pack.act(loc, action, label, PackValue { mode, pack_attr: true, n });
    }

    /// `#pragma options align=k` / `#pragma align=k` at `loc` (the
    /// `options` / `align` token); clang `ActOnPragmaOptionsAlign`.
    pub fn act_on_pragma_options_align(&mut self, kind: OptionsAlign, loc: Loc) {
        let push_set = PackAction { set: true, push: true, ..PackAction::default() };
        let (action, mode) = match kind {
            OptionsAlign::Native | OptionsAlign::Power => (push_set, AlignMode::Native),
            OptionsAlign::Natural => (push_set, AlignMode::Natural),
            OptionsAlign::Packed => (push_set, AlignMode::Packed),
            OptionsAlign::Mac68k => {
                let d = self.diag(DiagId::sema_pragma_options_align_mac68k_unsupported, loc);
                self.emit(d);
                return;
            }
            OptionsAlign::Reset => {
                let cur = self.pack.current;
                let pop = PackAction { pop: true, ..PackAction::default() };
                if !self.pack.stack.is_empty() {
                    (pop, AlignMode::Native)
                } else if cur.mode != AlignMode::Native || cur.pack_attr {
                    (PackAction::default(), AlignMode::Native)
                } else {
                    let d = self
                        .diag(DiagId::sema_pragma_options_align_reset_failed, loc)
                        .arg("stack empty");
                    self.emit(d);
                    return;
                }
            }
        };
        self.pack.act(loc, action, None, PackValue { mode, pack_attr: false, n: 0 });
    }

    /// The end of the translation unit: every push without its pop
    /// (clang `DiagnoseUnterminatedPragmaAlignPack`), the innermost
    /// first; when the value in effect is the default, the innermost
    /// also notes the pragma that reset it.
    pub(crate) fn diagnose_unterminated_pragma_pack(&mut self) {
        let reset = (self.pack.current == PackValue::default()).then_some(self.pack.current_loc);
        let slots: Vec<Loc> = self.pack.stack.iter().rev().map(|s| s.push_loc).collect();
        for (i, push) in slots.into_iter().enumerate() {
            let mut d = self.diag(DiagId::sema_pragma_pack_no_pop_eof, push);
            if i == 0
                && let Some(Some(at)) = reset
            {
                d = d.with_note(self.diag(DiagId::note_pragma_pack_pop_instead_reset, at));
            }
            self.emit(d);
        }
    }
}
