//! Lambda expressions: the side record of a `LambdaExpr` (spec/ast.md §5,
//! spec/sema-lambda.md; [expr.prim.lambda]).
//!
//! A `LambdaExpr` node's `a` is its [`LambdaId`]; the closure type's
//! `RecordData.lambda` names the same record, so the Oven and codegen reach
//! the captures from the class as well as from the expression.
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{DeclId, LambdaId, ListId, StmtId};
use crate::mem::MemRow;

/// The capture-default of a lambda-introducer ([expr.prim.lambda.capture]/1).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub enum CaptureDefault {
    /// None written.
    #[default]
    None,
    /// `=`.
    ByCopy,
    /// `&`.
    ByRef,
}

/// How one entity is captured ([expr.prim.lambda.capture]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub enum CaptureKind {
    /// `this` (the closure holds the pointer).
    This,
    /// `*this` (the closure holds a copy of the object).
    StarThis,
    /// A variable captured by copy (or an init-capture without `&`).
    ByCopy,
    /// A variable captured by reference (or an init-capture with `&`).
    ByRef,
}

/// One capture of a lambda, in closure member order: the explicit captures
/// as written, then the implicit ones in order of first odr-use.
#[derive(Copy, Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct LambdaCapture {
    /// How it is captured.
    pub kind: CaptureKind,
    /// The captured variable, parameter or binding — for an init-capture
    /// its own `VarDecl` (`None` for `this` / `*this`).
    pub var: Option<DeclId>,
    /// The unnamed non-static data member holding it (set when the closure
    /// is completed; `None` for a capture of a pack before expansion).
    pub field: Option<DeclId>,
    /// Where the capture was written (or first odr-used, when implicit).
    pub loc: Loc,
    /// The `...` of a capture pack ([expr.prim.lambda.capture]/17), or
    /// `Loc::INVALID`.
    pub ellipsis: Loc,
    /// Captured implicitly through a capture-default.
    pub implicit: bool,
    /// An init-capture ([expr.prim.lambda.capture]/6).
    pub init: bool,
}

/// The mangling context of a closure type ([basic.def.odr]/14, Itanium
/// ABI §5.1.8; clang's `getCurrentMangleNumberContext`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub struct LambdaMangling {
    /// The 1-based discriminator among the closures of the same context
    /// with the same call-operator signature (`Ul..E_` is 1, `Ul..E0_` is
    /// 2); 0 when the closure has no mangling number (it has internal
    /// linkage and mangles as `$_N`).
    pub number: u32,
    /// The declaration whose name prefixes the closure's when it is not
    /// its declaration context (a default argument's function, a
    /// variable or data member initialized by the lambda); `None` uses
    /// the context.
    pub context: Option<DeclId>,
}

/// The side record of a lambda ([expr.prim.lambda]).
#[derive(Clone, Debug, kiln_snap::Snap)]
#[allow(clippy::struct_excessive_bools, reason = "independent syntactic facts")]
pub struct LambdaData {
    /// The closure type (`CXXRecordDecl`, `DefinitionBits::LAMBDA`).
    pub closure: DeclId,
    /// The function call operator (`CXXMethodDecl`; for a generic lambda
    /// the pattern of the member function template), once declared.
    pub call_op: Option<DeclId>,
    /// The capture-default and where it was written.
    pub default: CaptureDefault,
    /// Where the capture-default was written.
    pub default_loc: Loc,
    /// The captures in closure member order.
    pub captures: Vec<LambdaCapture>,
    /// The initializer of each capture's member (same order), once
    /// complete: `ExprId`s.
    pub inits: Option<ListId>,
    /// The compound-statement (also the call operator's body).
    pub body: Option<StmtId>,
    /// The `[`.
    pub lbracket: Loc,
    /// The `]`.
    pub rbracket: Loc,
    /// A parameter-declaration-clause was written.
    pub explicit_params: bool,
    /// A trailing return type was written.
    pub explicit_result: bool,
    /// `mutable` was written.
    pub mutable: bool,
    /// The mangling number and context.
    pub mangling: LambdaMangling,
}

impl LambdaData {
    /// A record for closure `closure` whose introducer starts at
    /// `lbracket`.
    pub fn new(closure: DeclId, lbracket: Loc) -> Self {
        LambdaData {
            closure,
            call_op: None,
            default: CaptureDefault::None,
            default_loc: Loc::INVALID,
            captures: Vec::new(),
            inits: None,
            body: None,
            lbracket,
            rbracket: Loc::INVALID,
            explicit_params: false,
            explicit_result: false,
            mutable: false,
            mangling: LambdaMangling::default(),
        }
    }

    /// The capture whose member is `field`.
    pub fn capture_of_field(&self, field: DeclId) -> Option<&LambdaCapture> {
        self.captures.iter().find(|c| c.field == Some(field))
    }

    /// The capture of variable `var`.
    pub fn capture_of_var(&self, var: DeclId) -> Option<&LambdaCapture> {
        self.captures.iter().find(|c| c.var == Some(var) && !c.init)
    }

    /// The `this` or `*this` capture.
    pub fn this_capture(&self) -> Option<&LambdaCapture> {
        self.captures.iter().find(|c| matches!(c.kind, CaptureKind::This | CaptureKind::StarThis))
    }
}

/// The lambda side records. Index 0 is a dummy.
#[derive(Debug, kiln_snap::Snap)]
pub struct LambdaArena {
    records: Vec<LambdaData>,
}

impl Default for LambdaArena {
    fn default() -> Self {
        let dummy = DeclId::new(DeclId::MAX_INDEX);
        LambdaArena { records: vec![LambdaData::new(dummy, Loc::INVALID)] }
    }
}

impl LambdaArena {
    /// The largest lambda index (the snapshot range check).
    pub(crate) fn max_index(&self) -> usize {
        self.records.len().saturating_sub(1)
    }

    /// Adds a record.
    pub fn push(&mut self, x: LambdaData) -> LambdaId {
        let id = LambdaId::new(u32::try_from(self.records.len()).unwrap_or(u32::MAX));
        self.records.push(x);
        id
    }

    /// The record `id`.
    pub fn get(&self, id: LambdaId) -> &LambdaData {
        &self.records[id.index() as usize]
    }

    /// The record `id`, for the builder.
    pub fn get_mut(&mut self, id: LambdaId) -> &mut LambdaData {
        &mut self.records[id.index() as usize]
    }

    /// The number of lambdas.
    pub fn len(&self) -> usize {
        self.records.len() - 1
    }

    /// True if there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn mem_row(&self) -> MemRow {
        let caps: usize = self.records.iter().map(|r| r.captures.capacity()).sum();
        let mut row = MemRow::of_vec("lambdas", &self.records);
        let extra = (caps * size_of::<LambdaCapture>()) as u64;
        row.live_bytes += extra;
        row.reserved_bytes += extra;
        row
    }
}

/// A member of a closure type that has no body of its own: evaluation and
/// code generation synthesize it (clang `isLambdaStaticInvoker` and the
/// conversion's `EmitLambdaToBlockPointerBody`/`EmitLambdaStaticInvokeBody`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub enum ClosureThunk {
    /// The conversion function to pointer to function
    /// ([expr.prim.lambda.closure]/8): returns `target`, the static invoker
    /// or (for a static lambda) the call operator itself.
    Conversion {
        /// The function whose address is returned.
        target: DeclId,
    },
    /// The static invoker `__invoke`: forwards its arguments to the call
    /// operator `call_op` (whose body never uses `this`: the lambda has no
    /// captures).
    Invoker {
        /// The call operator.
        call_op: DeclId,
    },
}

impl crate::Ast {
    /// The lambda whose closure type is class `r`.
    pub fn closure_lambda(&self, r: DeclId) -> Option<LambdaId> {
        self.decls.record(self.first(r)).and_then(|x| x.lambda)
    }

    /// The lambda whose function call operator is `f`: the call operator
    /// itself or, for a generic lambda, a specialization of its template.
    pub fn call_op_lambda(&self, f: DeclId) -> Option<LambdaId> {
        let rec = self.decls.get(f).ctx()?;
        let lambda = self.closure_lambda(rec)?;
        let op = self.exprs.lambdas.get(lambda).call_op?;
        if self.first(op) == self.first(f) {
            return Some(lambda);
        }
        let (_, s) = self.spec_info(f)?;
        let t = self.described_template(op)?;
        (self.first(t) == self.first(s.template) || self.first(op) == self.first(s.from))
            .then_some(lambda)
    }

    /// The static invoker `__invoke` of closure type `r` (the one implicit
    /// static member function of a closure; for a generic lambda, the
    /// pattern of the invoker template).
    pub fn closure_invoker(&self, r: DeclId) -> Option<DeclId> {
        let l = self.closure_lambda(r)?;
        let call_op = self.exprs.lambdas.get(l).call_op;
        self.members(self.first(r))
            .map(|m| match self.decls.get(m).kind() {
                crate::decl::DeclKind::FunctionTemplate => {
                    self.template_info(m).and_then(|i| i.pattern).unwrap_or(m)
                }
                _ => m,
            })
            .find(|&m| {
                Some(m) != call_op
                    && self.decls.get(m).kind() == crate::decl::DeclKind::CXXMethod
                    && self.decls.get(m).flags().contains(crate::decl::DeclFlags::IMPLICIT)
                    && self
                        .decls
                        .function(m)
                        .is_some_and(|f| f.spec.storage() == crate::decl::FnStorage::Static)
            })
    }

    /// Whether function `d` is a body-less closure member, and which. A
    /// specialization of a generic lambda's conversion or invoker
    /// template forwards to the specializations for the same arguments
    /// (clang `SpecializeCorrespondingLambdaCallOperatorAndInvoker`);
    /// `None` while one of them does not exist.
    pub fn closure_thunk(&self, d: DeclId) -> Option<ClosureThunk> {
        let implicit =
            |x: DeclId| self.decls.get(x).flags().contains(crate::decl::DeclFlags::IMPLICIT);
        let (pat, args) = match self.spec_info(d) {
            Some((_, s)) => {
                let pat = self.template_info(s.template)?.pattern?;
                (pat, Some(self.types.canonical_args(s.args)))
            }
            None => (d, None),
        };
        if !implicit(pat) {
            return None;
        }
        let decl = self.decls.get(pat);
        let rec = decl.ctx()?;
        let lambda = self.closure_lambda(rec)?;
        let call_op = self.exprs.lambdas.get(lambda).call_op?;
        let invoker = self.closure_invoker(rec);
        let spec = |x: DeclId| match args {
            None => Some(x),
            Some(a) => {
                let t = self.described_template(x)?;
                self.templates.find_spec(self.first(t), a)
            }
        };
        match decl.kind() {
            crate::decl::DeclKind::CXXConversion => {
                let target = spec(invoker.unwrap_or(call_op))?;
                Some(ClosureThunk::Conversion { target })
            }
            crate::decl::DeclKind::CXXMethod
                if Some(self.first(pat)) == invoker.map(|i| self.first(i)) =>
            {
                Some(ClosureThunk::Invoker { call_op: spec(call_op)? })
            }
            _ => None,
        }
    }
}
