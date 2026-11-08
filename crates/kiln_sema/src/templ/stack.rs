//! The instantiation stack (spec/sema-templates.md §10): frames with
//! their points of instantiation, the depth limit, the clang-worded
//! context notes with backtrace elision, and the SFINAE context flag.

use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::ArgListId;
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHasher;

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
    /// The *noexcept-specifier* of a member of a specialization,
    /// instantiated when needed ([temp.inst]/14, WP-112).
    ExceptionSpec,
    /// The annotations of a function of a specialization, substituted
    /// when needed ([dcl.attr.annotation]/4; spec/annotations.md).
    Annotation,
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
    /// Deducing the arguments of a class template partial
    /// specialization while matching it ([temp.spec.partial.match]; not
    /// an instantiation). The frame's `args` are the deduced arguments.
    PartialSpecDeduction,
    /// The declaration of a variable template specialization
    /// ([temp.inst]/7).
    VarTemplateInstantiation,
    /// The initializer of a variable template specialization (charged
    /// to the specialization's report row without counting it again).
    VarTemplateInit,
    /// Substituting deduced arguments into a function template's
    /// signature ([temp.deduct.general]/5; a SFINAE context). The frame's
    /// `args` are the deduced arguments.
    DeducedArgSubstitution,
    /// Substituting explicitly-specified arguments into a function
    /// template's signature ([temp.arg.explicit]; a SFINAE context).
    ExplicitArgSubstitution,
    /// Checking the satisfaction of a template's associated constraints
    /// or a concept-id ([temp.constr.sat]; WP-073). Not an instantiation.
    ConstraintsCheck,
    /// Checking the satisfaction of a concept-id ([temp.names]/9).
    ConceptCheck,
    /// Substituting into an atomic constraint or a requirement
    /// ([temp.constr.atomic]/3, [expr.prim.req]; a SFINAE context).
    ConstraintSubstitution,
    /// Building a tuple-like structured binding's initializer
    /// ([dcl.struct.bind]/4; clang `InitializingStructuredBinding`). Not
    /// an instantiation; the entity is the binding.
    BindingInit,
    /// Substituting the prior arguments into a non-type parameter's type
    /// while checking its argument ([temp.arg.general]; clang
    /// `PriorTemplateArgumentSubstitution`). The entity is the
    /// parameter; the frame's `args` are the prior arguments.
    PriorArgSubstitution,
    /// Building a class template's implicit deduction guides
    /// ([over.match.class.deduct]/1; clang `BuildingDeductionGuides`).
    /// Not an instantiation; the point is the template name of the
    /// placeholder that first needed them.
    BuildingDeductionGuides,
    /// Matching a template template argument against its parameter
    /// under clang 22's rules ([temp.arg.template]/3-4; clang
    /// `PartialOrderingTTP`). Not an instantiation; the entity is the
    /// template template parameter, the point the argument. Prints two
    /// notes: the mismatch at the argument and the parameter.
    PartialOrderingTtp,
    /// Substituting into a lambda-expression's body while instantiating
    /// its enclosing template (clang `LambdaExpressionSubstitution`). Not
    /// an instantiation; the entity is the new call operator, the point
    /// the body's `{`. Ends a SFINAE context: the body is never in the
    /// immediate context (CWG2672, [temp.deduct.general]/9).
    LambdaSubstitution,
    /// Forming a template-id for `std::meta::substitute` or
    /// `std::meta::can_substitute` ([meta.reflection.substitute]; a
    /// SFINAE context). Not an instantiation; the entity and template are
    /// the substituted template.
    MetaSubstitution,
}

impl FrameKind {
    /// Whether the frame counts toward the depth limit.
    pub const fn is_instantiation(self) -> bool {
        !matches!(
            self,
            FrameKind::DefaultArgChecking
                | FrameKind::PartialSpecDeduction
                | FrameKind::ConstraintsCheck
                | FrameKind::ConceptCheck
                | FrameKind::ConstraintSubstitution
                | FrameKind::BindingInit
                | FrameKind::PriorArgSubstitution
                | FrameKind::BuildingDeductionGuides
                | FrameKind::PartialOrderingTtp
                | FrameKind::LambdaSubstitution
                | FrameKind::MetaSubstitution
        )
    }

    /// The context note clang prints for the frame.
    pub const fn note(self) -> DiagId {
        match self {
            FrameKind::ClassInstantiation => DiagId::note_template_class_instantiation_here,
            FrameKind::MemberClass => DiagId::note_template_member_class_here,
            FrameKind::MemberFunction => DiagId::note_template_member_function_here,
            FrameKind::ExceptionSpec => DiagId::note_template_exception_spec_here,
            FrameKind::Annotation => DiagId::note_template_annotation_here,
            FrameKind::FunctionSpec => DiagId::note_function_template_spec_here,
            FrameKind::StaticDataMember => DiagId::note_template_static_data_member_def_here,
            FrameKind::AliasTemplate => DiagId::note_template_type_alias_instantiation_here,
            FrameKind::Enum => DiagId::note_template_enum_def_here,
            FrameKind::Nsdmi => DiagId::note_template_nsdmi_here,
            FrameKind::DefaultTemplateArg => DiagId::note_default_arg_instantiation_here,
            FrameKind::DefaultFunctionArg => DiagId::note_default_function_arg_instantiation_here,
            FrameKind::DefaultArgChecking => DiagId::note_template_default_arg_checking,
            FrameKind::PriorArgSubstitution => DiagId::note_prior_template_arg_substitution,
            FrameKind::PartialSpecDeduction => DiagId::note_partial_spec_deduct_here,
            FrameKind::VarTemplateInstantiation | FrameKind::VarTemplateInit => {
                DiagId::note_var_template_spec_here
            }
            FrameKind::DeducedArgSubstitution => {
                DiagId::note_function_template_deduction_instantiation_here
            }
            FrameKind::ExplicitArgSubstitution => {
                DiagId::note_explicit_template_arg_substitution_here
            }
            FrameKind::ConstraintsCheck => DiagId::note_checking_constraints_here,
            FrameKind::ConceptCheck => DiagId::note_checking_concept_here,
            FrameKind::ConstraintSubstitution => DiagId::note_constraint_substitution_here,
            FrameKind::BindingInit => DiagId::note_in_binding_decl_init,
            FrameKind::BuildingDeductionGuides => DiagId::note_building_deduction_guide_here,
            FrameKind::PartialOrderingTtp => DiagId::note_template_arg_template_params_mismatch,
            FrameKind::LambdaSubstitution => DiagId::note_lambda_substitution_here,
            FrameKind::MetaSubstitution => DiagId::note_meta_substitution_here,
        }
    }

    /// The kind's name in the instantiation report (§11.3).
    pub const fn report_name(self) -> &'static str {
        match self {
            FrameKind::ClassInstantiation => "class",
            FrameKind::MemberClass => "member-class",
            FrameKind::MemberFunction => "member-function",
            FrameKind::ExceptionSpec => "exception-spec",
            FrameKind::Annotation => "annotation",
            FrameKind::FunctionSpec => "function",
            FrameKind::StaticDataMember => "static-data-member",
            FrameKind::AliasTemplate => "alias",
            FrameKind::Enum => "enum",
            FrameKind::Nsdmi => "nsdmi",
            FrameKind::DefaultTemplateArg => "default-template-arg",
            FrameKind::DefaultFunctionArg => "default-arg",
            FrameKind::DefaultArgChecking => "checking",
            FrameKind::PriorArgSubstitution => "prior-arg-substitution",
            FrameKind::PartialSpecDeduction => "partial-deduction",
            FrameKind::VarTemplateInstantiation => "var",
            FrameKind::VarTemplateInit => "var-init",
            FrameKind::DeducedArgSubstitution => "deduction",
            FrameKind::ExplicitArgSubstitution => "explicit-args",
            FrameKind::ConstraintsCheck => "constraints",
            FrameKind::ConceptCheck => "concept",
            FrameKind::ConstraintSubstitution => "constraint-subst",
            FrameKind::BindingInit => "binding-init",
            FrameKind::BuildingDeductionGuides => "deduction-guides",
            FrameKind::PartialOrderingTtp => "ttp-matching",
            FrameKind::LambdaSubstitution => "lambda",
            FrameKind::MetaSubstitution => "meta-substitute",
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
    /// Set only when the report is on (timing is opt-in, §11.3).
    pub(crate) start: Option<Instant>,
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
    /// The pattern member a class instantiation is substituting now
    /// (`DeclId::index`, 0 = none), mixed into [`Self::context_key`]: clang
    /// substitutes each declarator of `typedef typename T::type g, (*h)(...)`
    /// on its own and reports a shared decl-specifier error once per
    /// declarator ([temp.inst]/3; spec/sema-templates.md §10.2).
    pub(crate) member_salt: u32,
    /// The position of the braced-init-list element being initialized,
    /// hashed with the enclosing lists' (0 = none), mixed into
    /// [`Self::context_key`]: the elements of an expanded pack
    /// `{a[I]...}` share one location, and clang reports each one
    /// ([temp.variadic]/8; spec/sema.md §9).
    pub(crate) element_salt: u64,
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

    /// clang's `inTemplateInstantiation`: a frame that is an
    /// instantiation, or a constraint-satisfaction check of a template or
    /// concept-id (clang's `ConstraintsCheck` is an instantiation record
    /// although kiln's [`Self::depth`] does not count it).
    pub fn in_instantiation_record(&self) -> bool {
        self.depth() > 0
            || self
                .frames
                .iter()
                .any(|f| matches!(f.kind, FrameKind::ConstraintsCheck | FrameKind::ConceptCheck))
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

    /// The diagnostic deduplication context for the current stack
    /// (spec/diag.md §7.4): a hash of every frame's kind, entity,
    /// arguments and point of instantiation; 0 when the stack is empty.
    /// Two instantiations of one pattern hash differently, so an error in
    /// the pattern prints once per instantiation.
    pub(crate) fn context_key(&self) -> u64 {
        if self.frames.is_empty() {
            return 0;
        }
        let mut h = FxHasher::default();
        for f in &self.frames {
            (f.kind, f.entity, f.args, f.poi).hash(&mut h);
        }
        if self.member_salt != 0 {
            self.member_salt.hash(&mut h);
        }
        if self.element_salt != 0 {
            self.element_salt.hash(&mut h);
        }
        // 0 means "no context"; keep it reserved.
        h.finish().max(1)
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
    /// depth was already printed (§10.2). `name` renders an entity;
    /// `note` picks the frame's note.
    pub(crate) fn context_notes(
        &self,
        limit: u32,
        name: &dyn Fn(&InstFrame) -> Option<String>,
        note: &dyn Fn(&InstFrame) -> DiagId,
        extra: &dyn Fn(&InstFrame) -> Option<Diag>,
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
            let id = note(f);
            let mut d = Diag::new(id, kiln_source::Span::empty(f.poi));
            if id.arg_count() == 1 {
                d = d.arg(name(f).unwrap_or_default());
            }
            out.push(d);
            out.extend(extra(f));
        }
        out
    }
}

impl Sema<'_, '_> {
    /// Whether instantiation has stopped for the rest of the translation
    /// unit: a fatal error was emitted, by the depth limit or by anything
    /// else (the error limit's "too many errors emitted", a missing
    /// `#include`, `-Wfatal-errors`). clang's `InstantiatingTemplate`
    /// constructor makes every instantiation invalid once
    /// `Diags.hasFatalErrorOccurred() && hasUncompilableErrorOccurred()`
    /// (a fatal error is uncompilable): every later diagnostic is dropped,
    /// so the work could not be seen (spec/sema.md "Error recovery",
    /// WP-826).
    pub(crate) fn instantiation_stopped(&mut self) -> bool {
        if !self.inst.fatal && self.diags.has_fatal() {
            self.inst.fatal = true;
        }
        self.inst.fatal
    }

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
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("push_inst_frame");
        if self.instantiation_stopped() {
            return false;
        }
        if self.stack_exhausted() || self.stack_low() {
            // P4 (c): the frame that would not fit is part of the
            // backtrace, as for the depth limit.
            self.inst.stack.push(InstFrame {
                kind,
                entity,
                template,
                poi,
                sfinae: None,
                args: None,
                start: None,
                start_bytes: 0,
                start_nodes: 0,
                child_ns: 0,
            });
            let _ = self.stack_ok(crate::stack_guard::StackPhase::Instantiation, poi);
            let _ = self.inst.stack.pop();
            self.inst.fatal = true;
            return false;
        }
        let sfinae = match kind {
            FrameKind::ClassInstantiation
            | FrameKind::MemberClass
            | FrameKind::MemberFunction
            | FrameKind::ExceptionSpec
            | FrameKind::Annotation
            | FrameKind::FunctionSpec
            | FrameKind::StaticDataMember
            | FrameKind::VarTemplateInstantiation
            | FrameKind::VarTemplateInit
            | FrameKind::Enum
            | FrameKind::Nsdmi
            | FrameKind::DefaultFunctionArg
            | FrameKind::LambdaSubstitution => Some(false),
            FrameKind::DeducedArgSubstitution
            | FrameKind::ExplicitArgSubstitution
            | FrameKind::MetaSubstitution => Some(true),
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
                start: None,
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
            start: self.inst.report.timed().then(Instant::now),
            start_bytes,
            start_nodes,
            child_ns: 0,
        });
        true
    }

    /// Pops the innermost frame, charging its meter to its template's
    /// report row (§11.3).
    pub(crate) fn pop_inst_frame(&mut self) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("pop_inst_frame");
        let Some(f) = self.inst.stack.pop() else { return };
        if let Some((depth, slot)) = &mut self.inst.drain_capture
            && *depth == self.inst.stack.len()
        {
            *slot = Some(f.clone());
        }
        self.ast.end_instantiation();
        let incl = f.start.map_or(0, |t| u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX));
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
        self.inst.stack.context_notes(
            self.inst.limits.backtrace,
            &|f| Some(self.frame_name(f)),
            &|f| self.frame_note(f),
            &|f| self.frame_extra_note(f),
        )
    }

    /// The second note of a frame that prints two: a template template
    /// argument's matching names the parameter (clang
    /// `note_template_prev_declaration`).
    fn frame_extra_note(&self, f: &InstFrame) -> Option<Diag> {
        (f.kind == FrameKind::PartialOrderingTtp).then(|| {
            Diag::new(
                DiagId::note_template_prev_declaration,
                kiln_source::Span::empty(self.ast.decls.get(f.entity).loc()),
            )
            .arg(1u8)
        })
    }

    /// The note a frame prints: its kind's, except that a variable
    /// template specialization that is a class member is a static data
    /// member (clang `PrintInstantiationStack`: `VD->isStaticDataMember()`
    /// picks `note_template_static_data_member_def_here`).
    fn frame_note(&self, f: &InstFrame) -> DiagId {
        match f.kind {
            FrameKind::VarTemplateInstantiation | FrameKind::VarTemplateInit
                if self
                    .ast
                    .decls
                    .get(f.entity)
                    .ctx()
                    .is_some_and(|c| self.kind_of(c) == DeclKind::CXXRecord) =>
            {
                DiagId::note_template_static_data_member_def_here
            }
            k => k.note(),
        }
    }

    /// Whether the note of variable template specialization frame `f`
    /// names the pattern (`'v'`, `'u<T *>'`) rather than the
    /// specialization: clang's `BuildVarTemplateInstantiation` charges
    /// the declaration's instantiation (and the initializer that deduces
    /// an undeduced type there) to the pattern, except for a member
    /// template of a class template specialization.
    fn var_frame_names_pattern(&self, f: &InstFrame) -> bool {
        if self.inst_template(f.template) != f.template {
            return false;
        }
        f.kind == FrameKind::VarTemplateInstantiation
            || self.ast.decls.get(f.entity).ty().is_some_and(|t| self.contains_undeduced_auto(t))
    }

    /// How a frame's entity prints in its note: a class by its type
    /// (`A<int>`), anything else by its qualified name.
    fn frame_name(&self, f: &InstFrame) -> String {
        match f.kind {
            FrameKind::ClassInstantiation => {
                // clang: `Context.getTypeDeclType(Record)`, scoped.
                let t = self.ast.decls.get(f.entity).ty().unwrap_or(kiln_ast::QualType::ERROR);
                self.decl_ty_str(t)
            }
            FrameKind::DefaultTemplateArg if f.args.is_some() => {
                let mut s = self.qualified_decl_name(f.template);
                if let Some(l) = f.args {
                    let names = crate::names::SemaNames::new(self);
                    kiln_ast::types::write_arg_list(&self.ast.types, &names, l, &mut s);
                }
                s
            }
            FrameKind::PartialSpecDeduction => self.partial_spec_desc(f.entity, f.args),
            FrameKind::PriorArgSubstitution => {
                // clang: `%select{non-type|template}0 template
                // parameter%1 %2`, the name as ` 'V'` when there is one.
                let name = self.decl_name(f.entity);
                let name = if name.is_empty() { String::new() } else { format!(" '{name}'") };
                let with = f.args.map(|l| self.template_args_desc(f.template, l));
                format!("non-type template parameter{name} {}", with.unwrap_or_default())
            }
            FrameKind::ConstraintsCheck | FrameKind::ConceptCheck => match f.args {
                Some(l) => self.spec_name(f.template, l),
                None => self.qualified_decl_name(f.template),
            },
            // clang streams the `FunctionTemplateDecl` itself: unqualified.
            // clang shows the explicit substitution's (still empty)
            // deduced-argument list: nothing after the name but a space.
            FrameKind::ExplicitArgSubstitution => format!("'{}' ", self.decl_name(f.template)),
            FrameKind::MetaSubstitution => format!("'{}'", self.qualified_decl_name(f.template)),
            FrameKind::DeducedArgSubstitution => {
                let name = self.decl_name(f.template);
                match f.args {
                    Some(l) => format!("'{name}' {}", self.template_args_desc(f.template, l)),
                    None => format!("'{name}'"),
                }
            }
            FrameKind::FunctionSpec => match self.ast.spec_info(f.entity) {
                Some((_, i)) => self.function_spec_name(f.entity, i.template, i.args),
                None => self.qualified_decl_name(f.entity),
            },
            FrameKind::VarTemplateInstantiation | FrameKind::VarTemplateInit => {
                match self.ast.spec_info(f.entity) {
                    Some((_, i)) if self.var_frame_names_pattern(f) => {
                        match self.ast.partial_spec_info(i.from) {
                            Some((_, p)) => self.spec_name(p.template, p.args),
                            None => self.qualified_decl_name(i.from),
                        }
                    }
                    Some((_, i)) => self.spec_decl_name(i.template, i.print_args),
                    None => self.qualified_decl_name(f.entity),
                }
            }
            // clang: the unqualified name, then the innermost template
            // arguments (`f<B>` for a member of `A<B>`; an exception
            // specification's frame has them only for a function template
            // specialization).
            FrameKind::DefaultFunctionArg | FrameKind::ExceptionSpec | FrameKind::Annotation => {
                let mut s = self.decl_name(f.entity);
                if let Some(l) = f.args {
                    let names = crate::names::SemaNames::new(self);
                    kiln_ast::types::write_arg_list(&self.ast.types, &names, l, &mut s);
                }
                s
            }
            // clang names the function unqualified here.
            FrameKind::BindingInit => self.decl_name(f.entity),
            // A local class (or a class nested in one) is named as a
            // declaration, through its function (`f()::L::N`).
            FrameKind::MemberClass => crate::names::SemaNames::new(self)
                .local_class_decl_name(f.entity)
                .unwrap_or_else(|| self.qualified_decl_name(f.entity)),
            _ => self.qualified_decl_name(f.entity),
        }
    }

    /// The number of errors the innermost SFINAE context has captured so
    /// far (0 outside one). A substitution that returns `Ok` while this
    /// count grew failed all the same (a narrowing conversion diagnosed
    /// inside a `decltype`): its result must not be memoized as valid.
    pub(crate) fn sfinae_errors(&self) -> u32 {
        self.inst.sfinae.borrow().last().map_or(0, |s| s.errors)
    }

    /// The number of warnings the innermost SFINAE context holds back.
    pub(crate) fn sfinae_warnings(&self) -> usize {
        self.inst.sfinae.borrow().last().map_or(0, |s| s.warnings.len())
    }

    /// The warnings the innermost SFINAE context held back after the
    /// first `from`, taken out of it.
    pub(crate) fn take_sfinae_warnings(&self, from: usize) -> Vec<Diag> {
        let mut st = self.inst.sfinae.borrow_mut();
        match st.last_mut() {
            Some(s) if s.warnings.len() > from => s.warnings.split_off(from),
            _ => Vec::new(),
        }
    }

    /// Reports the warnings held back while function template
    /// specialization `d`'s signature was substituted, at its first use
    /// (clang `DiagnoseUseOfDecl`; §5.7).
    pub(crate) fn emit_suppressed_diags(&mut self, d: DeclId, loc: Loc) {
        if self.templ.suppressed_diags.is_empty() {
            return;
        }
        let ds = self.take_suppressed_diags(d, loc);
        if !ds.is_empty() {
            let _ = self.templ.deduction_diags.remove(&d);
            self.emit_held_warnings(ds);
        }
    }

    /// The warnings specialization `d` holds back for its first use, at
    /// `loc`, taken. The entry stays, empty: a later deduction forming
    /// the same specialization adds nothing to it (clang keeps it too).
    /// Warnings an earlier use that found no match handed back have their
    /// context notes moved from that use to `loc` from clang 22 on (clang
    /// stores only a successful deduction's, made here; WP-424).
    pub(crate) fn take_suppressed_diags(&mut self, d: DeclId, loc: Loc) -> Vec<Diag> {
        let mut ds =
            self.templ.suppressed_diags.get_mut(&d).map(std::mem::take).unwrap_or_default();
        if let Some(from) = self.special.dropped_tid_warnings.remove(&d)
            && self.clang_compat >= 22
        {
            for x in &mut ds {
                x.relocate_notes(from, loc);
            }
        }
        ds
    }

    /// Reports the held-back substitution warnings `ds`
    /// ([`Sema::emit_suppressed_diags`]).
    pub(crate) fn emit_held_warnings(&mut self, ds: Vec<Diag>) {
        for x in ds {
            // From clang 22 on they carry their own context notes and
            // take none of the use's.
            if self.clang_compat >= 22 && !self.inst.stack.is_sfinae() {
                x.emit(self.diags);
            } else {
                self.emit(x);
            }
        }
    }

    /// Keeps the warnings `held` back by the phases of a deduction that
    /// formed `spec` before its signature was substituted (the explicit
    /// arguments', the defaults'), ahead of the signature's own (clang
    /// `FinishTemplateArgumentDeduction`: the deduction's diagnostics are
    /// stored only when `spec` has no entry yet; `fresh`, this deduction
    /// formed it, and made that entry). How many lead the entry is
    /// recorded: a use that finds no match hands back only the rest
    /// ([`Sema::drop_template_id_warnings`]).
    pub(crate) fn keep_deduction_warnings(&mut self, spec: DeclId, fresh: bool, held: Vec<Diag>) {
        if held.is_empty() {
            return;
        }
        // An entry a use that found no match handed back holds only the
        // signature's, waiting, like clang's empty slot, for a deduction
        // that succeeds: this one's go ahead of them.
        let fresh = fresh || self.special.dropped_tid_warnings.contains_key(&spec);
        let n = held.len();
        match self.templ.suppressed_diags.entry(spec) {
            std::collections::hash_map::Entry::Vacant(v) => {
                let _ = v.insert(held);
            }
            std::collections::hash_map::Entry::Occupied(mut o) if fresh => {
                let own = std::mem::replace(o.get_mut(), held);
                o.get_mut().extend(own);
            }
            std::collections::hash_map::Entry::Occupied(_) => return,
        }
        let _ = self.templ.deduction_diags.insert(spec, n);
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
    /// The warnings held back, each with its context notes from clang 22
    /// on (§5.7): a specialization that is formed keeps them for its
    /// first use ([`Sema::emit_suppressed_diags`]).
    pub warnings: Vec<Diag>,
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
            start: None,
            start_bytes: 0,
            start_nodes: 0,
            child_ns: 0,
        }
    }

    #[test]
    fn notes_print_once_per_depth() {
        let mut s = InstantiationStack::default();
        s.push(frame(FrameKind::ClassInstantiation, 1));
        assert_eq!(
            s.context_notes(10, &|_| Some("A<int>".into()), &|f| f.kind.note(), &|_| None).len(),
            1
        );
        assert!(s.context_notes(10, &|_| None, &|f| f.kind.note(), &|_| None).is_empty());
        s.push(frame(FrameKind::MemberFunction, 2));
        assert_eq!(s.context_notes(10, &|_| None, &|f| f.kind.note(), &|_| None).len(), 2);
        let _ = s.pop();
        // Popped below the printed depth: prints again.
        assert_eq!(s.context_notes(10, &|_| None, &|f| f.kind.note(), &|_| None).len(), 1);
    }

    #[test]
    fn elision_keeps_head_and_tail() {
        let mut s = InstantiationStack::default();
        for i in 0..15 {
            s.push(frame(FrameKind::ClassInstantiation, i + 1));
        }
        // 5 + skip note + 5.
        assert_eq!(s.context_notes(10, &|_| None, &|f| f.kind.note(), &|_| None).len(), 11);
        s.last_emitted.set(0);
        assert_eq!(s.context_notes(0, &|_| None, &|f| f.kind.note(), &|_| None).len(), 15);
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
