//! The instantiation stack (spec/sema-templates.md §10): frames with
//! their points of instantiation, the depth limit, the clang-worded
//! context notes with backtrace elision, and the SFINAE context flag.

use std::cell::Cell;
use std::time::Instant;

use kiln_ast::DeclId;
use kiln_ast::ids::ArgListId;
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;

/// What a frame instantiates (§10.1). The "while …" kinds are not
/// instantiations: they print a note but do not count toward the depth
/// limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameKind {
    /// The definition of a class template specialization.
    ClassInstantiation,
    /// The definition of a member class of a specialization.
    MemberClass,
    /// The definition of a member function of a specialization.
    MemberFunction,
    /// The definition of a function template specialization (WP-071).
    FunctionSpec,
    /// The initializer of a static data member of a specialization.
    StaticDataMember,
    /// An alias template specialization's type.
    AliasTemplate,
    /// The definition of a member enumeration.
    Enum,
    /// A default member initializer.
    Nsdmi,
    /// A default template argument.
    DefaultTemplateArg,
    /// A default function argument.
    DefaultFunctionArg,
    /// Checking a default template argument (not an instantiation).
    DefaultArgChecking,
}

impl FrameKind {
    /// Whether the frame counts toward the depth limit.
    pub const fn is_instantiation(self) -> bool {
        !matches!(self, FrameKind::DefaultArgChecking)
    }

    /// The context note clang prints for the frame.
    pub const fn note(self) -> DiagId {
        match self {
            FrameKind::ClassInstantiation => DiagId::note_template_class_instantiation_here,
            FrameKind::MemberClass => DiagId::note_template_member_class_here,
            FrameKind::MemberFunction => DiagId::note_template_member_function_here,
            FrameKind::FunctionSpec => DiagId::note_function_template_spec_here,
            FrameKind::StaticDataMember => DiagId::note_template_static_data_member_def_here,
            FrameKind::AliasTemplate => DiagId::note_template_type_alias_instantiation_here,
            FrameKind::Enum => DiagId::note_template_enum_def_here,
            FrameKind::Nsdmi => DiagId::note_template_nsdmi_here,
            FrameKind::DefaultTemplateArg => DiagId::note_default_arg_instantiation_here,
            FrameKind::DefaultFunctionArg => DiagId::note_default_function_arg_instantiation_here,
            FrameKind::DefaultArgChecking => DiagId::note_template_default_arg_checking,
        }
    }

    /// The kind's name in the instantiation report (§11.3).
    pub const fn report_name(self) -> &'static str {
        match self {
            FrameKind::ClassInstantiation => "class",
            FrameKind::MemberClass => "member-class",
            FrameKind::MemberFunction => "member-function",
            FrameKind::FunctionSpec => "function",
            FrameKind::StaticDataMember => "static-data-member",
            FrameKind::AliasTemplate => "alias",
            FrameKind::Enum => "enum",
            FrameKind::Nsdmi => "nsdmi",
            FrameKind::DefaultTemplateArg => "default-template-arg",
            FrameKind::DefaultFunctionArg => "default-arg",
            FrameKind::DefaultArgChecking => "checking",
        }
    }
}

/// One frame (§10.1) with its report meter (§11.3).
#[derive(Clone, Debug)]
pub struct InstFrame {
    /// What is instantiated.
    pub kind: FrameKind,
    /// The entity (specialization, member, alias template).
    pub entity: DeclId,
    /// The template whose report row the frame charges.
    pub template: DeclId,
    /// Where the instantiation was required.
    pub poi: Loc,
    /// `Some(true)` for a SFINAE context, `Some(false)` for a frame that
    /// ends one (a definition instantiated from inside SFINAE), `None`
    /// to inherit.
    pub sfinae: Option<bool>,
    /// The arguments converted so far, for a default-argument frame
    /// (its note names `D<void>`).
    pub args: Option<ArgListId>,
    pub(crate) start: Instant,
    pub(crate) start_bytes: u64,
    pub(crate) start_nodes: u64,
    pub(crate) child_ns: u64,
}

/// The depth and backtrace limits (`-ftemplate-depth=`,
/// `-ftemplate-backtrace-limit=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstLimits {
    /// Maximum instantiation depth (clang's default 1024).
    pub depth: u32,
    /// Frames printed before eliding (default 10; 0 = unlimited).
    pub backtrace: u32,
}

impl Default for InstLimits {
    fn default() -> Self {
        InstLimits { depth: 1024, backtrace: 10 }
    }
}

/// The frames, innermost last (§10.1).
#[derive(Debug, Default)]
pub struct InstantiationStack {
    pub(crate) frames: Vec<InstFrame>,
    /// Frames that are not instantiations.
    pub(crate) non_inst: u32,
    /// The stack depth at which the context was last printed; 0 = never
    /// (clang's `LastEmittedCodeSynthesisContextDepth`).
    pub(crate) last_emitted: Cell<u32>,
}

impl InstantiationStack {
    /// Number of frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The instantiation depth (frames that count toward the limit).
    pub fn depth(&self) -> u32 {
        self.frames.len() as u32 - self.non_inst
    }

    /// Whether diagnostics are currently in a SFINAE context (the
    /// innermost frame that says).
    pub fn is_sfinae(&self) -> bool {
        self.frames.iter().rev().find_map(|f| f.sfinae).unwrap_or(false)
    }

    /// Whether `entity` is being instantiated as `kind` (a cycle).
    pub fn contains(&self, kind: FrameKind, entity: DeclId) -> bool {
        self.frames.iter().any(|f| f.kind == kind && f.entity == entity)
    }

    pub(crate) fn push(&mut self, f: InstFrame) {
        if !f.kind.is_instantiation() {
            self.non_inst += 1;
        }
        self.frames.push(f);
    }

    pub(crate) fn pop(&mut self) -> Option<InstFrame> {
        let f = self.frames.pop()?;
        if !f.kind.is_instantiation() {
            self.non_inst -= 1;
        }
        // Popping the frame where the context was printed: the next
        // diagnostic prints it again (clang's `popCodeSynthesisContext`).
        if self.last_emitted.get() as usize > self.frames.len() {
            self.last_emitted.set(0);
        }
        Some(f)
    }

    /// The context notes for a diagnostic emitted now, or none when this
    /// depth was already printed (§10.2). `name` renders an entity.
    pub(crate) fn context_notes(
        &self,
        limit: u32,
        name: &dyn Fn(&InstFrame) -> Option<String>,
    ) -> Vec<Diag> {
        let n = self.frames.len() as u32;
        if n == 0 || self.last_emitted.get() == n {
            return Vec::new();
        }
        self.last_emitted.set(n);
        let mut out = Vec::new();
        // Innermost first, as clang prints them.
        let order: Vec<&InstFrame> = self.frames.iter().rev().collect();
        let (skip_lo, skip_hi) = if limit != 0 && n > limit {
            let head = limit.div_ceil(2);
            let tail = limit / 2;
            (head, n - tail)
        } else {
            (n, n)
        };
        for (i, f) in order.iter().enumerate() {
            let i = i as u32;
            if i == skip_lo && skip_hi > skip_lo {
                let k = skip_hi - skip_lo;
                out.push(
                    Diag::new(
                        DiagId::note_instantiation_contexts_suppressed,
                        kiln_source::Span::empty(f.poi),
                    )
                    .arg(k)
                    .arg(if k == 1 { "" } else { "s" }),
                );
            }
            if i >= skip_lo && i < skip_hi {
                continue;
            }
            let id = f.kind.note();
            let mut d = Diag::new(id, kiln_source::Span::empty(f.poi));
            if id.arg_count() == 1 {
                d = d.arg(name(f).unwrap_or_default());
            }
            out.push(d);
        }
        out
    }
}

impl Sema<'_, '_> {
    /// Pushes an instantiation frame ([temp.inst]/16): false, after the
    /// fatal depth diagnostic (once per translation unit), when the
    /// depth limit is reached or a fatal error already stopped
    /// instantiation.
    pub(crate) fn push_inst_frame(
        &mut self,
        kind: FrameKind,
        entity: DeclId,
        template: DeclId,
        poi: Loc,
    ) -> bool {
        if self.inst.fatal {
            return false;
        }
        let sfinae = match kind {
            FrameKind::ClassInstantiation
            | FrameKind::MemberClass
            | FrameKind::MemberFunction
            | FrameKind::FunctionSpec
            | FrameKind::StaticDataMember
            | FrameKind::Enum
            | FrameKind::Nsdmi => Some(false),
            _ => None,
        };
        if kind.is_instantiation() && self.inst.stack.depth() >= self.inst.limits.depth {
            // As clang, the frame that exceeds the limit is part of the
            // backtrace (`R<1024>` with the default depth).
            self.inst.stack.push(InstFrame {
                kind,
                entity,
                template,
                poi,
                sfinae,
                args: None,
                start: Instant::now(),
                start_bytes: 0,
                start_nodes: 0,
                child_ns: 0,
            });
            let note = self.diag(DiagId::note_template_recursion_depth, poi);
            let d = self
                .diag(DiagId::sema_template_recursion_depth, poi)
                .arg(self.inst.limits.depth)
                .with_note(note);
            self.emit(d);
            let _ = self.inst.stack.pop();
            self.inst.fatal = true;
            return false;
        }
        if kind.is_instantiation() {
            self.inst.report.open();
        }
        let start_bytes = self.ast.node_live_bytes();
        let start_nodes = self.ast.node_count();
        self.ast.begin_instantiation();
        self.inst.stack.push(InstFrame {
            kind,
            entity,
            template,
            poi,
            sfinae,
            args: None,
            start: Instant::now(),
            start_bytes,
            start_nodes,
            child_ns: 0,
        });
        true
    }

    /// Pops the innermost frame, charging its meter to its template's
    /// report row (§11.3).
    pub(crate) fn pop_inst_frame(&mut self) {
        let Some(f) = self.inst.stack.pop() else { return };
        self.ast.end_instantiation();
        let incl = u64::try_from(f.start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let bytes = self.ast.node_live_bytes().saturating_sub(f.start_bytes);
        let nodes = self.ast.node_count().saturating_sub(f.start_nodes);
        if let Some(parent) = self.inst.stack.frames.last_mut() {
            parent.child_ns = parent.child_ns.saturating_add(incl);
        }
        if f.kind.is_instantiation() {
            self.inst.report.charge(f.kind, f.template, nodes, bytes, incl, f.child_ns);
        }
    }

    /// The context notes the next diagnostic carries (§10.2).
    pub(crate) fn inst_context_notes(&self) -> Vec<Diag> {
        self.inst.stack.context_notes(self.inst.limits.backtrace, &|f| Some(self.frame_name(f)))
    }

    /// How a frame's entity prints in its note: a class by its type
    /// (`A<int>`), anything else by its qualified name.
    fn frame_name(&self, f: &InstFrame) -> String {
        match f.kind {
            FrameKind::ClassInstantiation => {
                let t = self.ast.decls.get(f.entity).ty().unwrap_or(kiln_ast::QualType::ERROR);
                self.ty_str(t)
            }
            FrameKind::DefaultTemplateArg if f.args.is_some() => {
                let mut s = self.qualified_decl_name(f.template);
                if let Some(l) = f.args {
                    let names = crate::names::SemaNames::new(self);
                    kiln_ast::types::write_arg_list(&self.ast.types, &names, l, &mut s);
                }
                s
            }
            _ => self.qualified_decl_name(f.entity),
        }
    }

    /// Records the arguments converted so far on the innermost frame.
    pub(crate) fn set_frame_args(&mut self, args: ArgListId) {
        if let Some(f) = self.inst.stack.frames.last_mut() {
            f.args = Some(args);
        }
    }
}

/// How errors are handled (§5.7): reported, or captured by an enclosing
/// SFINAE context.
#[derive(Debug, Default)]
pub struct SfinaeState {
    /// The first error captured (the deduction-failure note's text).
    pub first: Option<Diag>,
    /// Number of errors captured.
    pub errors: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: FrameKind, poi: u32) -> InstFrame {
        InstFrame {
            kind,
            entity: DeclId::new(1),
            template: DeclId::new(1),
            poi: Loc::from_raw(poi),
            sfinae: None,
            args: None,
            start: Instant::now(),
            start_bytes: 0,
            start_nodes: 0,
            child_ns: 0,
        }
    }

    #[test]
    fn notes_print_once_per_depth() {
        let mut s = InstantiationStack::default();
        s.push(frame(FrameKind::ClassInstantiation, 1));
        assert_eq!(s.context_notes(10, &|_| Some("A<int>".into())).len(), 1);
        assert!(s.context_notes(10, &|_| None).is_empty());
        s.push(frame(FrameKind::MemberFunction, 2));
        assert_eq!(s.context_notes(10, &|_| None).len(), 2);
        let _ = s.pop();
        // Popped below the printed depth: prints again.
        assert_eq!(s.context_notes(10, &|_| None).len(), 1);
    }

    #[test]
    fn elision_keeps_head_and_tail() {
        let mut s = InstantiationStack::default();
        for i in 0..15 {
            s.push(frame(FrameKind::ClassInstantiation, i + 1));
        }
        // 5 + skip note + 5.
        assert_eq!(s.context_notes(10, &|_| None).len(), 11);
        s.last_emitted.set(0);
        assert_eq!(s.context_notes(0, &|_| None).len(), 15);
    }

    #[test]
    fn checking_frames_do_not_count() {
        let mut s = InstantiationStack::default();
        s.push(frame(FrameKind::ClassInstantiation, 1));
        s.push(frame(FrameKind::DefaultArgChecking, 2));
        assert_eq!(s.depth(), 1);
        let _ = s.pop();
        assert_eq!(s.depth(), 1);
    }
}
