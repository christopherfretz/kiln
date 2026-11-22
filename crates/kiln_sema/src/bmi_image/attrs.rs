//! Declaration attributes and builtin identity in the declaration image
//! (spec/modules.md §11.11, "Attributes", WP-1278).
//!
//! Every attribute node on a carried declaration is one [`AttrRec`]: the
//! node's kind, syntax, flags and location, and its two argument words
//! written as the kind's checked operand roles say
//! (`kiln_ast::roles::attr_roles`, [`op_of`]). The roles are the whole
//! format: a kind they give no [`Op`], or a node whose words fall outside
//! what its roles allow, makes the declaration fall back
//! ([`UNCARRIED`]). An expression argument is an expression record of the
//! bodies section ([`super::body`], `Root::Attr`), a `Symbol` argument a
//! string of the section, a builtin id the builtin's spelling as well as
//! its id (checked against the importer's table). The written spelling
//! sema keeps for diagnostics (`Sema::attr_spellings`) travels with the
//! node.
//!
//! The importer attaches the nodes right after it builds the declaration,
//! in the order the unit had them, and fills in the expression arguments
//! once the bodies are built. Its consumers read the attribute list on
//! demand (codegen visibility and linkage, mangling, the use-site
//! warnings), so an imported declaration behaves as the replayed one.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::builtin;
use kiln_ast::expr::{ExprKind, Radix};
use kiln_ast::ids::RawId;
use kiln_ast::roles::{self, Role};
use kiln_ast::strings::StringEncoding;
use kiln_ast::{AttrId, DeclId, ExprId, ListId, StringLitId};
use kiln_source::symbol::Symbol;

use super::body::Root;
use super::locs::Locs;
use super::{Image, NONE, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// The census reason of an attribute the table does not carry.
pub(super) const UNCARRIED: &str = "an attribute the image does not carry yet";

/// How a kind's argument words are carried.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    /// Plain words, at most these values (`a`, `b`).
    Words(u32, u32),
    /// `a` is a `Symbol` (written as its text), `b` a plain word at
    /// most this value (`builtin_alias`'s spelling, WP-1806).
    Sym(u32),
    /// `a` and `b` are each an expression or zero.
    Exprs,
    /// `a` is an expression or zero, `b` zero or a type (`aligned(E)`,
    /// `alignas(type-id)`; the type since WP-1345, written as an index
    /// into [`Attrs::types`]).
    ExprA,
    /// `a` zero, `b` a type (`preferred_name(typedef-name)`, WP-1345),
    /// written as an index into [`Attrs::types`].
    Type,
    /// `a` is a builtin id (written with its spelling), `b` zero.
    Builtin,
    /// `a` is a list of integer literals of type `size_t` (`nonnull`'s
    /// parameter indices; the empty list names every parameter), `b`
    /// zero. Written as a run of [`Attrs::ints`].
    Ints,
    /// `a` is the ordinary string literal of an asm label (written as
    /// its text), `b` zero. A written label (not one inherited from a
    /// previous declaration, flag 2) also names the first declaration's
    /// symbol (`cold.asm_label`).
    Label,
    /// WP-1806: `a` is an ordinary string literal (`StringLitId`, written
    /// as its text) or zero, `b` a plain word at most this value
    /// (`section`, `target`, `weakref`, `#pragma clang section`'s kind).
    Lit(u32),
    /// WP-1806: `a` is an ordinary string literal, `b` a raw location
    /// (`alias`, `ifunc`: one past the `)`; `target_version`: the
    /// literal's), written as an image location.
    LitLoc,
    /// WP-1806: `a` is a function declared by an earlier record (written
    /// as that record's index), `b` a raw location (`cleanup`'s checked
    /// form).
    DeclLoc,
    /// WP-1806: `a` is an expression or zero, `b` zero or the `ListId`
    /// of more expressions (`annotate("s", args...)`), written as a run
    /// of [`Attrs::xexprs`] (`s` its start, `b` its length).
    ExprList,
    /// WP-1806: `a` is the `ListId` of raw words (`callback`'s checked
    /// argument indices), `b` zero, written as a run of [`Attrs::words`]
    /// (`a` its start, `b` its length).
    RawList,
}

/// Whether an attribute of kind `kind` with words `a`, `b` is one an
/// explicit instantiation's attribute vector holds: plain words, or an
/// expression kind written without its expressions (WP-1353:
/// `__attribute__((__deprecated__))` on libc++'s `extern template class
/// codecvt_byname<char16_t, ...>`, [dcl.attr.deprecated]).
fn inst_word(kind: AttrKind, a: u32, b: u32) -> bool {
    match op_of(kind) {
        Some(Op::Words(am, bm)) => a <= am && b <= bm,
        Some(Op::Exprs | Op::ExprList) => a == 0 && b == 0,
        _ => false,
    }
}

const ANY: u32 = u32::MAX;

/// The declaration-attribute kinds the image leaves out, each with the
/// reason: none of them is ever an attribute node of a declaration the
/// image writes. Every other kind but the type attributes is carried as
/// its roles say ([`op_of`]; `every_kind_is_carried_or_exempt`).
const NOT_CARRIED: &[(AttrKind, &str)] = &[
    (AttrKind::ANNOTATION, "moved into `Ast::annots` by sema (the image's annotations)"),
    (AttrKind::COMMON, "rejected in C++, never attached"),
    (AttrKind::CPU_DISPATCH, "never attached (invalid on AArch64, unsupported on x86)"),
    (AttrKind::CPU_SPECIFIC, "never attached (invalid on AArch64, unsupported on x86)"),
    (AttrKind::ASSUME, "a statement attribute (`body.rs` STMT_ATTRS, else the body falls back)"),
    (AttrKind::FALLTHROUGH, "a statement attribute, carried by `body.rs` STMT_ATTRS"),
    (AttrKind::LIKELY, "a statement attribute, carried by `body.rs` STMT_ATTRS"),
    (AttrKind::UNLIKELY, "a statement attribute, carried by `body.rs` STMT_ATTRS"),
    (AttrKind::MUSTTAIL, "a statement attribute (`body.rs` STMT_ATTRS, else the body falls back)"),
    (AttrKind::LOOP_HINT, "a statement attribute (`body.rs` STMT_ATTRS, else the body falls back)"),
    (
        AttrKind::CODE_ALIGN,
        "a statement attribute (`body.rs` STMT_ATTRS, else the body falls back)",
    ),
];

/// How kind `k`'s words are carried: derived from its checked roles
/// (`kiln_ast::roles::attr_roles`, the operand-role table tooling clients
/// read), so the image and the table cannot drift. `None` for a type
/// attribute, a [`NOT_CARRIED`] kind, or roles no [`Op`] writes.
fn op_of(k: AttrKind) -> Option<Op> {
    if k.is_vector_type() || k.is_fn_type_attr() || NOT_CARRIED.iter().any(|&(t, _)| t == k) {
        return None;
    }
    let r = roles::attr_roles(k)?;
    op_from(r.a.resolve(roles::Words::CHECKED_ATTR), r.b.resolve(roles::Words::CHECKED_ATTR))
}

/// The [`Op`] of the checked roles `a`, `b`.
fn op_from(a: &Role, b: &Role) -> Option<Op> {
    use Role as R;
    let expr = |r: Role| matches!(r, R::Expr | R::Opt(&R::Expr));
    Some(match (*a, *b) {
        (R::Zero, R::Type) => Op::Type,
        (R::Builtin, R::Zero) => Op::Builtin,
        (R::List(&R::Expr), R::Zero) => Op::Ints,
        (R::List(&R::Value(_)), R::Zero) => Op::RawList,
        (R::StringLit, R::Zero) => Op::Label,
        (R::StringLit, R::Loc) => Op::LitLoc,
        (R::Opt(&R::StringLit), b) => Op::Lit(word_max(&b)?),
        (R::Decl, R::Loc) => Op::DeclLoc,
        (R::Symbol, b) => Op::Sym(word_max(&b)?),
        (R::Opt(&R::Expr), R::Opt(&R::List(&R::Expr))) => Op::ExprList,
        (R::Opt(&R::Expr), R::Opt(&R::Type)) => Op::ExprA,
        (a, b) if expr(a) && (expr(b) || b == R::Zero) => Op::Exprs,
        (a, b) => Op::Words(word_max(&a)?, word_max(&b)?),
    })
}

/// The largest value a plain word of role `r` holds ([`ANY`] for any).
fn word_max(r: &Role) -> Option<u32> {
    match *r {
        Role::Zero => Some(0),
        Role::Bool => Some(1),
        Role::Enum { max, .. } => Some(max),
        Role::Value(_) => Some(ANY),
        Role::Bits(fs) => fs.iter().try_fold(0, |m, f| word_max(f.role).map(|_| m | f.mask())),
        _ => None,
    }
}

/// Whether the image carries `node` (its kind and argument words).
pub(super) fn carried(node: &AttrNode, nsyms: usize) -> bool {
    match op_of(node.kind) {
        None => false,
        Some(Op::Sym(bm)) => (node.a as usize) < nsyms && node.b <= bm,
        // A priority still dependent holds its expression.
        Some(Op::Words(..))
            if node.flags & AttrNode::PRIORITY_DEPENDENT != 0
                && matches!(node.kind, AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR) =>
        {
            false
        }
        Some(Op::Words(am, bm)) => node.a <= am && node.b <= bm,
        Some(Op::Exprs | Op::ExprA | Op::ExprList) => true,
        Some(Op::Builtin) => node.b == 0 && builtin::info(node.a).is_some(),
        Some(Op::Type) => node.a == 0 && node.b != 0,
        Some(Op::Ints) => node.b == 0,
        // A `callback` list is never empty.
        Some(Op::Label | Op::RawList) => node.b == 0 && node.a != 0,
        Some(Op::Lit(bm)) => node.b <= bm,
        Some(Op::LitLoc | Op::DeclLoc) => node.a != 0,
    }
}

fn syntax_of(b: u8) -> Option<AttrSyntax> {
    Some(match b {
        0 => AttrSyntax::Cxx11,
        1 => AttrSyntax::Gnu,
        2 => AttrSyntax::Keyword,
        3 => AttrSyntax::Declspec,
        4 => AttrSyntax::Pragma,
        _ => return None,
    })
}

/// The text of ordinary string literal `raw` (a `StringLitId`), or the
/// census reason: another encoding, or bytes that are not UTF-8, are not
/// carried.
fn ordinary_text(s: &Sema<'_, '_>, raw: u32) -> Result<String, &'static str> {
    let lit = StringLitId::from_raw(raw).ok_or(UNCARRIED)?;
    let (enc, bytes) = s.ast.strings.get(lit);
    (enc == StringEncoding::Ordinary)
        .then(|| std::str::from_utf8(bytes).ok())
        .flatten()
        .map(str::to_owned)
        .ok_or(UNCARRIED)
}

/// One attribute node of record `decl`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct AttrRec {
    pub(super) decl: u32,
    kind: u16,
    syntax: u8,
    flags: u8,
    loc: u32,
    /// The words; for [`Op::Exprs`] expression record indices or
    /// [`NONE`]; for [`Op::Ints`] the first of the run of
    /// [`Attrs::ints`] and its length.
    pub(super) a: u32,
    pub(super) b: u32,
    /// [`Op::Sym`], [`Op::Label`], [`Op::Lit`], [`Op::LitLoc`]: the
    /// text; [`Op::Builtin`]: the builtin's spelling (an index into
    /// [`Attrs::strs`]); [`Op::ExprList`]: the start of its run of
    /// [`Attrs::xexprs`]; else [`NONE`].
    s: u32,
    /// The written scope and name (`Sema::attr_spellings`), or [`NONE`].
    scope: u32,
    name: u32,
}

/// The builtin identity of a function record: which cold map (0
/// `builtin_id`, 1 `lib_builtin`, 2 `gnu_lib_twin`), the id and its
/// spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct BuiltinRec {
    decl: u32,
    map: u8,
    id: u32,
    name: u32,
}

/// The image's attribute section.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Attrs {
    strs: Vec<String>,
    /// Ascending by record.
    pub(super) recs: Vec<AttrRec>,
    builtins: Vec<BuiltinRec>,
    /// The integer literals of [`Op::Ints`] arguments: value and
    /// location.
    ints: Vec<(u32, u32)>,
    /// WP-1314: the attributes of function parameters (`decl` the
    /// function record) and the parameter's position, ascending; only
    /// kinds whose words are plain ([`Op::Words`]: `lifetimebound`,
    /// `noescape`, `maybe_unused`).
    pub(super) params: Vec<(u32, AttrRec)>,
    /// WP-1345: the type arguments of [`Op::Type`] and [`Op::ExprA`]
    /// nodes.
    pub(super) types: Vec<super::QRef>,
    /// WP-1345: the attributes written on an explicit instantiation
    /// (`decl` its index in the explicit instantiation section),
    /// ascending; only kinds whose words are plain ([`Op::Words`]:
    /// `visibility`).
    pub(super) insts: Vec<AttrRec>,
    /// WP-1806: the further expression arguments of [`Op::ExprList`]
    /// nodes, as expression records.
    pub(super) xexprs: Vec<u32>,
    /// WP-1806: the words of [`Op::RawList`] nodes.
    words: Vec<u32>,
}

/// What the writer computed for one declaration, committed with its
/// record ([`Writer::attr_commit`]).
#[derive(Default)]
pub(super) struct Pending {
    recs: Vec<(AttrRec, [Option<ExprId>; 2])>,
    builtins: Vec<BuiltinRec>,
    ints: Vec<Vec<(u32, u32)>>,
    /// The type argument of each record, in order (WP-1345).
    types: Vec<Option<super::QRef>>,
    /// The further expressions of each [`Op::ExprList`] record, in order
    /// (WP-1806).
    xlists: Vec<Vec<ExprId>>,
    /// The words of each [`Op::RawList`] record, in order (WP-1806).
    words: Vec<Vec<u32>>,
}

impl Writer<'_, '_, '_> {
    /// Whether every attribute of `d` is one the image carries.
    pub(super) fn attrs_carried(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.symbols().len();
        ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).all(|a| carried(ast.attrs.get(a), n))
    }

    /// Whether `d`'s symbol (`cold.asm_label`, keyed by the first
    /// declaration) is an asm label written on a declaration of `d`'s
    /// entity, whose attribute record carries it. The writing
    /// declaration may be a later one (glibc's `__REDIRECT` redeclares a
    /// prototype with a label, WP-1310): it is in the image too, since a
    /// redeclaration whose previous declaration is outside the image is
    /// refused, and the reader re-keys its label on the first.
    pub(super) fn own_asm_label(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let Some(&lit) = ast.decls.cold.asm_label.get(d) else { return true };
        let own = |r: DeclId| {
            ast.lists.iter::<AttrId>(ast.attrs.of_decl(r)).any(|a| {
                let n = ast.attrs.get(a);
                n.kind == AttrKind::ASM_LABEL && n.flags & 2 == 0 && n.a == lit.raw()
            })
        };
        own(d) || (ast.decls.redecl(d).is_some() && ast.redecls_of(d).any(own))
    }

    /// Records the attributes of parameter `p`, position `k` of function
    /// record `rec` (WP-1314; [`Attrs::params`]).
    pub(super) fn param_attrs(&mut self, p: DeclId, rec: u32, k: u32) -> W<()> {
        let s = self.s;
        let ast = &s.ast;
        let nsyms = ast.symbols().len();
        for a in ast.lists.iter::<AttrId>(ast.attrs.of_decl(p)) {
            let node = *ast.attrs.get(a);
            if !matches!(op_of(node.kind), Some(Op::Words(..))) || !carried(&node, nsyms) {
                return Err(UNCARRIED);
            }
            let loc = self.loc(node.loc)?;
            let (scope, name) = self.attr_spelling(node.loc);
            let r = AttrRec {
                decl: rec,
                kind: node.kind.0,
                syntax: node.syntax as u8,
                flags: node.flags,
                loc,
                a: node.a,
                b: node.b,
                s: NONE,
                scope,
                name,
            };
            self.img.attrs.params.push((k, r));
        }
        Ok(())
    }

    /// Records attribute `node`, written on explicit instantiation
    /// `inst` (WP-1345; [`Attrs::insts`]).
    pub(super) fn inst_attr(&mut self, node: AttrNode, inst: u32) -> W<()> {
        let nsyms = self.s.ast.symbols().len();
        if !inst_word(node.kind, node.a, node.b) || !carried(&node, nsyms) {
            return Err(UNCARRIED);
        }
        let loc = self.loc(node.loc)?;
        let (scope, name) = self.attr_spelling(node.loc);
        self.img.attrs.insts.push(AttrRec {
            decl: inst,
            kind: node.kind.0,
            syntax: node.syntax as u8,
            flags: node.flags,
            loc,
            a: node.a,
            b: node.b,
            s: NONE,
            scope,
            name,
        });
        Ok(())
    }

    /// The written scope and name of the attribute at `at`, as string
    /// indices, or [`NONE`].
    fn attr_spelling(&mut self, at: kiln_source::Loc) -> (u32, u32) {
        let s = self.s;
        let ast = &s.ast;
        match s.attr_spellings.get(&at) {
            Some(&(sc, nm)) if at.is_valid() => {
                let sc = sc.map(|x| ast.symbols().as_str(x).to_owned());
                let nm = ast.symbols().as_str(nm).to_owned();
                (sc.map_or(NONE, |x| self.attr_str(&x)), self.attr_str(&nm))
            }
            _ => (NONE, NONE),
        }
    }

    fn attr_str(&mut self, s: &str) -> u32 {
        if let Some(i) = self.img.attrs.strs.iter().position(|x| x == s) {
            return i as u32;
        }
        self.img.attrs.strs.push(s.to_owned());
        self.img.attrs.strs.len() as u32 - 1
    }

    /// The attribute records and builtin identity of `d`, record `rec`
    /// (checked by [`Writer::attrs_carried`] in `head`).
    #[allow(clippy::too_many_lines)] // one arm per table op, kept together with the format
    pub(super) fn attr_recs(&mut self, d: DeclId, rec: u32) -> W<Pending> {
        let s = self.s;
        let ast = &s.ast;
        let mut p = Pending::default();
        for (map, id) in [
            (0u8, ast.decls.builtin_id(d)),
            (1, ast.decls.lib_builtin(d)),
            (2, ast.decls.gnu_lib_twin(d)),
        ] {
            let Some(id) = id else { continue };
            let info = builtin::info(id).ok_or("a builtin the image does not know")?;
            let name = self.attr_str(info.name);
            p.builtins.push(BuiltinRec { decl: rec, map, id, name });
        }
        let nsyms = ast.symbols().len();
        for a in ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)) {
            let node = *ast.attrs.get(a);
            let op = op_of(node.kind).filter(|_| carried(&node, nsyms)).ok_or(UNCARRIED)?;
            let loc = self.loc(node.loc)?;
            let (scope, name) = self.attr_spelling(node.loc);
            let mut r = AttrRec {
                decl: rec,
                kind: node.kind.0,
                syntax: node.syntax as u8,
                flags: node.flags,
                loc,
                a: node.a,
                b: node.b,
                s: NONE,
                scope,
                name,
            };
            let mut exprs = [None, None];
            let mut ty = None;
            match op {
                Op::Words(..) => {}
                Op::Type => {
                    let q = kiln_ast::QualType::from_raw(node.b).ok_or(UNCARRIED)?;
                    ty = Some(self.qref(q)?);
                    r.b = NONE;
                }
                Op::Sym(_) => {
                    let t = ast.symbols().as_str(Symbol::from_index(node.a)).to_owned();
                    r.s = self.attr_str(&t);
                    r.a = 0;
                }
                Op::Lit(_) => {
                    if node.a != 0 {
                        let text = ordinary_text(s, node.a)?;
                        r.s = self.attr_str(&text);
                    }
                    r.a = 0;
                }
                Op::LitLoc => {
                    let text = ordinary_text(s, node.a)?;
                    r.s = self.attr_str(&text);
                    r.a = 0;
                    r.b = self.loc(kiln_source::Loc::from_raw(node.b))?;
                }
                Op::DeclLoc => {
                    let f = DeclId::from_raw(node.a).ok_or(UNCARRIED)?;
                    if !ast.decls.get(f).kind().is_function() {
                        return Err(UNCARRIED);
                    }
                    r.a =
                        *self.recs.get(&f).ok_or("a cleanup function the image does not carry")?;
                    r.b = self.loc(kiln_source::Loc::from_raw(node.b))?;
                }
                Op::ExprList => {
                    exprs = [ExprId::try_new(node.a), None];
                    let list = ListId::from_raw(node.b).filter(|_| node.b != 0);
                    let more: Vec<ExprId> =
                        list.map(|l| ast.lists.iter::<ExprId>(l).collect()).unwrap_or_default();
                    (r.a, r.b) = (NONE, more.len() as u32);
                    p.xlists.push(more);
                }
                Op::RawList => {
                    let list = ListId::from_raw(node.a).ok_or(UNCARRIED)?;
                    let w = ast.lists.get(list).to_vec();
                    (r.a, r.b) = (0, w.len() as u32);
                    p.words.push(w);
                }
                Op::Exprs => {
                    exprs = [ExprId::try_new(node.a), ExprId::try_new(node.b)];
                    (r.a, r.b) = (NONE, NONE);
                }
                Op::ExprA => {
                    exprs = [ExprId::try_new(node.a), None];
                    if let Some(q) = kiln_ast::QualType::from_raw(node.b) {
                        ty = Some(self.qref(q)?);
                    }
                    (r.a, r.b) = (NONE, NONE);
                }
                Op::Builtin => {
                    let info = builtin::info(node.a).ok_or(UNCARRIED)?;
                    r.s = self.attr_str(info.name);
                }
                Op::Ints => {
                    let list = ListId::from_raw(node.a).unwrap_or(ListId::EMPTY);
                    let mut run = Vec::new();
                    for e in ast.lists.iter::<ExprId>(list) {
                        let n = ast.exprs.get(e);
                        let v = (n.kind() == ExprKind::IntegerLiteral && n.ty() == s.size_type())
                            .then(|| ast.exprs.int_value(e))
                            .flatten()
                            .and_then(|v| u32::try_from(v).ok())
                            .ok_or(UNCARRIED)?;
                        run.push((v, self.loc(n.loc())?));
                    }
                    (r.a, r.b) = (p.ints.len() as u32, run.len() as u32);
                    p.ints.push(run);
                }
                Op::Label => {
                    let text = ordinary_text(s, node.a)?;
                    r.s = self.attr_str(&text);
                    r.a = 0;
                }
            }
            p.recs.push((r, exprs));
            p.types.push(ty);
        }
        Ok(p)
    }

    /// Adds `p` to the image, once its declaration's record is in.
    pub(super) fn attr_commit(&mut self, rec: u32, p: Pending) {
        let mut runs = p.ints.into_iter();
        let mut xlists = p.xlists.into_iter();
        let mut words = p.words.into_iter();
        for ((mut r, exprs), ty) in p.recs.into_iter().zip(p.types) {
            if let Some(q) = ty {
                r.b = self.img.attrs.types.len() as u32;
                self.img.attrs.types.push(q);
            }
            let k = self.img.attrs.recs.len() as u32;
            if op_of(AttrKind(r.kind)) == Some(Op::Ints) {
                let run = runs.next().unwrap_or_default();
                r.a = self.img.attrs.ints.len() as u32;
                self.img.attrs.ints.extend(run);
            }
            match op_of(AttrKind(r.kind)) {
                Some(Op::ExprList) => {
                    let more = xlists.next().unwrap_or_default();
                    r.s = self.img.attrs.xexprs.len() as u32;
                    for e in more {
                        let x = self.img.attrs.xexprs.len() as u32;
                        self.img.attrs.xexprs.push(NONE);
                        self.bs.roots.push(Root::Attr(rec, x, 2, e));
                    }
                }
                Some(Op::RawList) => {
                    let w = words.next().unwrap_or_default();
                    r.a = self.img.attrs.words.len() as u32;
                    self.img.attrs.words.extend(w);
                }
                _ => {}
            }
            self.img.attrs.recs.push(r);
            for (slot, e) in (0u8..).zip(exprs) {
                if let Some(e) = e {
                    self.bs.roots.push(Root::Attr(rec, k, slot, e));
                }
            }
        }
        self.img.attrs.builtins.extend(p.builtins);
    }
}

pub(super) fn encode_attrs(w: &mut Wr, a: &Attrs) {
    w.len(a.strs.len());
    for s in &a.strs {
        w.str(s);
    }
    w.len(a.recs.len());
    for r in &a.recs {
        encode_rec(w, r);
    }
    w.len(a.builtins.len());
    for b in &a.builtins {
        w.u32(b.decl);
        w.u8(b.map);
        w.u32(b.id);
        w.u32(b.name);
    }
    w.len(a.ints.len());
    for &(v, loc) in &a.ints {
        w.u32(v);
        w.u32(loc);
    }
    w.len(a.params.len());
    for (k, r) in &a.params {
        w.u32(*k);
        encode_rec(w, r);
    }
    w.len(a.types.len());
    for &q in &a.types {
        w.q(q);
    }
    w.len(a.insts.len());
    for r in &a.insts {
        encode_rec(w, r);
    }
    w.len(a.xexprs.len());
    for &x in &a.xexprs {
        w.u32(x);
    }
    w.len(a.words.len());
    for &x in &a.words {
        w.u32(x);
    }
}

fn encode_rec(w: &mut Wr, r: &AttrRec) {
    w.u32(r.decl);
    w.u16(r.kind);
    w.u8(r.syntax);
    w.u8(r.flags);
    for x in [r.loc, r.a, r.b, r.s, r.scope, r.name] {
        w.u32(x);
    }
}

fn decode_rec(r: &mut Rd<'_>) -> Res<AttrRec> {
    let decl = r.u32()?;
    let kind = r.u16()?;
    let syntax = r.u8()?;
    let flags = r.u8()?;
    let [loc, x, y, s, scope, name] = [r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?];
    Ok(AttrRec { decl, kind, syntax, flags, loc, a: x, b: y, s, scope, name })
}

pub(super) fn decode_attrs(r: &mut Rd<'_>) -> Res<Attrs> {
    let mut a = Attrs::default();
    let n = r.count(4)?;
    for _ in 0..n {
        a.strs.push(r.str()?);
    }
    let n = r.count(32)?;
    for _ in 0..n {
        a.recs.push(decode_rec(r)?);
    }
    let n = r.count(13)?;
    for _ in 0..n {
        let decl = r.u32()?;
        let map = r.u8()?;
        let id = r.u32()?;
        let name = r.u32()?;
        a.builtins.push(BuiltinRec { decl, map, id, name });
    }
    let n = r.count(8)?;
    for _ in 0..n {
        a.ints.push((r.u32()?, r.u32()?));
    }
    let n = r.count(36)?;
    for _ in 0..n {
        let k = r.u32()?;
        a.params.push((k, decode_rec(r)?));
    }
    let n = r.count(5)?;
    for _ in 0..n {
        a.types.push(r.q()?);
    }
    let n = r.count(32)?;
    for _ in 0..n {
        a.insts.push(decode_rec(r)?);
    }
    let n = r.count(4)?;
    for _ in 0..n {
        a.xexprs.push(r.u32()?);
    }
    let n = r.count(4)?;
    for _ in 0..n {
        a.words.push(r.u32()?);
    }
    Ok(a)
}

/// Checks every index of the attribute section.
#[allow(clippy::too_many_lines)] // one check per table op, in format order
pub(super) fn validate_attrs(img: &Image) -> Res<()> {
    let a = &img.attrs;
    let str_ok = |i: u32| (i as usize) < a.strs.len();
    let spelled = |i: u32, id: u32| {
        str_ok(i) && builtin::info(id).is_some_and(|b| b.name == a.strs[i as usize])
    };
    let nexpr = img.bodies.exprs.len();
    let expr_ok = |i: u32| i == NONE || (i as usize) < nexpr;
    let ty_ok = |i: u32| (i as usize) < a.types.len();
    let mut last = 0;
    for r in &a.recs {
        if (r.decl as usize) >= img.decls.len() || r.decl < last {
            return bad("bad attribute record");
        }
        last = r.decl;
        let ok = syntax_of(r.syntax).is_some()
            && (r.scope == NONE || str_ok(r.scope))
            && (r.name == NONE || str_ok(r.name))
            && (r.scope == NONE || r.name != NONE)
            && match op_of(AttrKind(r.kind)) {
                None => false,
                Some(Op::Words(am, bm)) => r.a <= am && r.b <= bm && r.s == NONE,
                Some(Op::Sym(bm)) => r.a == 0 && r.b <= bm && str_ok(r.s),
                Some(Op::Label) => r.a == 0 && r.b == 0 && str_ok(r.s),
                Some(Op::Lit(bm)) => r.a == 0 && r.b <= bm && (r.s == NONE || str_ok(r.s)),
                Some(Op::LitLoc) => r.a == 0 && str_ok(r.s),
                // A function record before the variable's.
                Some(Op::DeclLoc) => {
                    r.s == NONE
                        && r.a < r.decl
                        && matches!(
                            img.decls.get(r.a as usize).map(|d| &d.payload),
                            Some(Payload::Function { .. })
                        )
                }
                Some(Op::ExprList) => {
                    expr_ok(r.a)
                        && (r.s as usize)
                            .checked_add(r.b as usize)
                            .is_some_and(|e| e <= a.xexprs.len())
                }
                Some(Op::RawList) => {
                    r.s == NONE
                        && (r.a as usize)
                            .checked_add(r.b as usize)
                            .is_some_and(|e| e <= a.words.len())
                }
                Some(Op::Exprs) => expr_ok(r.a) && expr_ok(r.b) && r.s == NONE,
                Some(Op::ExprA) => expr_ok(r.a) && (r.b == NONE || ty_ok(r.b)) && r.s == NONE,
                Some(Op::Type) => r.a == 0 && ty_ok(r.b) && r.s == NONE,
                Some(Op::Builtin) => r.b == 0 && spelled(r.s, r.a),
                Some(Op::Ints) => {
                    r.s == NONE
                        && (r.a as usize)
                            .checked_add(r.b as usize)
                            .is_some_and(|e| e <= a.ints.len())
                }
            };
        if !ok {
            return bad("bad attribute record");
        }
    }
    if a.xexprs.iter().any(|&x| (x as usize) >= nexpr) {
        return bad("bad attribute record");
    }
    let mut last = (0, 0);
    for &(k, ref r) in &a.params {
        let nparams = match img.decls.get(r.decl as usize).map(|d| &d.payload) {
            Some(Payload::Function { params, .. }) => params.len(),
            _ => 0,
        };
        let ok = (r.decl, k) >= last
            && (k as usize) < nparams
            && syntax_of(r.syntax).is_some()
            && (r.scope == NONE || str_ok(r.scope))
            && (r.name == NONE || str_ok(r.name))
            && (r.scope == NONE || r.name != NONE)
            && r.s == NONE
            && matches!(op_of(AttrKind(r.kind)), Some(Op::Words(am, bm)) if r.a <= am && r.b <= bm);
        if !ok {
            return bad("bad parameter attribute record");
        }
        last = (r.decl, k);
    }
    let mut last = 0;
    for r in &a.insts {
        let ok = r.decl >= last
            && super::explinst::inst_count(img) > r.decl as usize
            && syntax_of(r.syntax).is_some()
            && (r.scope == NONE || str_ok(r.scope))
            && (r.name == NONE || str_ok(r.name))
            && (r.scope == NONE || r.name != NONE)
            && r.s == NONE
            && inst_word(AttrKind(r.kind), r.a, r.b);
        if !ok {
            return bad("bad explicit instantiation attribute record");
        }
        last = r.decl;
    }
    for b in &a.builtins {
        let func = img
            .decls
            .get(b.decl as usize)
            .is_some_and(|d| matches!(d.payload, Payload::Function { .. }));
        if !func || b.map > 2 || !spelled(b.name, b.id) {
            return bad("bad builtin record");
        }
    }
    Ok(())
}

/// The expression arguments the importer fills in once the bodies are
/// built: (attribute, `a` record, `b` record, the raw `b` word kept when
/// `b` is no record, and for [`Op::ExprList`] the run of
/// [`Attrs::xexprs`] that becomes `b`'s list, empty for none).
pub(super) type AttrExprs = Vec<(AttrId, u32, u32, u32, (u32, u32))>;

impl Sema<'_, '_> {
    /// Attaches record `rec`'s attributes and builtin identity to `d`,
    /// noting the expression arguments in `pending`.
    #[allow(clippy::too_many_arguments)] // the reader's state, one part each
    pub(super) fn image_attrs(
        &mut self,
        img: &Image,
        rec: u32,
        d: DeclId,
        recs: &[DeclId],
        main: &Locs,
        pending: &mut AttrExprs,
        qt: &mut dyn FnMut(&mut Self, super::QRef) -> kiln_ast::QualType,
    ) {
        let a = &img.attrs;
        let lo = a.builtins.partition_point(|b| b.decl < rec);
        for b in a.builtins[lo..].iter().take_while(|b| b.decl == rec) {
            let cold = &mut self.ast.decls.cold;
            let map = match b.map {
                0 => &mut cold.builtin_id,
                1 => &mut cold.lib_builtin,
                _ => &mut cold.gnu_lib_twin,
            };
            let _ = map.insert(d, b.id);
        }
        let lo = a.recs.partition_point(|r| r.decl < rec);
        let mut ids = Vec::new();
        for r in a.recs[lo..].iter().take_while(|r| r.decl == rec) {
            let kind = AttrKind(r.kind);
            let loc = main.get(r.loc);
            let syntax = syntax_of(r.syntax).unwrap_or_default();
            // WP-1345: a type argument, built now (its typedef or class
            // is an earlier record).
            let ty = match op_of(kind) {
                Some(Op::Type | Op::ExprA) if r.b != NONE => {
                    qt(self, a.types[r.b as usize]).to_raw()
                }
                _ => 0,
            };
            if kind == AttrKind::DIAGNOSE_IF {
                self.any_diagnose_if = true;
            }
            let lit = |s: &mut Self, i: u32| {
                s.ast.strings.intern(StringEncoding::Ordinary, a.strs[i as usize].as_bytes()).raw()
            };
            let (x, y) = match op_of(kind) {
                Some(Op::Sym(_)) => (self.intern(&a.strs[r.s as usize]).index(), r.b),
                Some(Op::Exprs | Op::ExprList) => (0, 0),
                Some(Op::Lit(_)) => (if r.s == NONE { 0 } else { lit(self, r.s) }, r.b),
                Some(Op::LitLoc) => (lit(self, r.s), main.get(r.b).to_raw()),
                Some(Op::DeclLoc) => (recs[r.a as usize].raw(), main.get(r.b).to_raw()),
                Some(Op::RawList) => {
                    let w = &a.words[r.a as usize..r.a as usize + r.b as usize];
                    (self.ast.lists.push_raw(w).raw(), 0)
                }
                Some(Op::ExprA | Op::Type) => (0, ty),
                Some(Op::Ints) => {
                    let ty = self.size_type();
                    let run = &a.ints[r.a as usize..r.a as usize + r.b as usize];
                    let mut lits = Vec::with_capacity(run.len());
                    for &(v, at) in run {
                        let at = main.get(at);
                        lits.push(self.builder().int_literal(ty, u64::from(v), Radix::Decimal, at));
                    }
                    (self.ast.lists.push(&lits).raw(), 0)
                }
                Some(Op::Label) => {
                    let text = a.strs[r.s as usize].as_bytes();
                    let lit = self.ast.strings.intern(StringEncoding::Ordinary, text);
                    if r.flags & 2 == 0 {
                        let first = self.ast.first(d);
                        let _ = self.ast.decls.cold.asm_label.insert(first, lit);
                    }
                    (lit.raw(), 0)
                }
                _ => (r.a, r.b),
            };
            let id =
                self.ast.attrs.push(AttrNode { kind, syntax, flags: r.flags, loc, a: x, b: y });
            match op_of(kind) {
                Some(Op::Exprs) if r.a != NONE || r.b != NONE => {
                    pending.push((id, r.a, r.b, 0, (0, 0)));
                }
                Some(Op::ExprList) => pending.push((id, r.a, NONE, 0, (r.s, r.b))),
                // The type stays in `b` (WP-1345).
                Some(Op::ExprA) if r.a != NONE => {
                    pending.push((id, r.a, NONE, ty, (0, 0)));
                    if kind == AttrKind::ALIGNED {
                        self.image_aligned_pending(id);
                    }
                }
                _ => {}
            }
            if r.name != NONE && loc.is_valid() {
                let scope = (r.scope != NONE).then(|| self.intern(&a.strs[r.scope as usize]));
                let name = self.intern(&a.strs[r.name as usize]);
                self.attr_spellings.insert(loc, (scope, name));
            }
            ids.push(id);
        }
        self.builder().append_attrs(d, &ids);
    }

    /// Attaches the attributes of parameter `k` of function record `rec`
    /// to `p` (WP-1314).
    pub(super) fn image_param_attrs(
        &mut self,
        img: &Image,
        rec: u32,
        k: u32,
        p: DeclId,
        main: &Locs,
    ) {
        let a = &img.attrs;
        let lo = a.params.partition_point(|(j, r)| (r.decl, *j) < (rec, k));
        let mut ids = Vec::new();
        for (_, r) in a.params[lo..].iter().take_while(|(j, r)| r.decl == rec && *j == k) {
            let loc = main.get(r.loc);
            let syntax = syntax_of(r.syntax).unwrap_or_default();
            let node =
                AttrNode { kind: AttrKind(r.kind), syntax, flags: r.flags, loc, a: r.a, b: r.b };
            ids.push(self.ast.attrs.push(node));
            if r.name != NONE && loc.is_valid() {
                let scope = (r.scope != NONE).then(|| self.intern(&a.strs[r.scope as usize]));
                let name = self.intern(&a.strs[r.name as usize]);
                self.attr_spellings.insert(loc, (scope, name));
            }
        }
        if !ids.is_empty() {
            self.builder().append_attrs(p, &ids);
        }
    }

    /// Attaches the attributes written on explicit instantiation `inst`
    /// to its specialization `spec` (WP-1345), after the instantiation,
    /// as the parser's declaration does.
    pub(super) fn image_inst_attrs(&mut self, img: &Image, inst: u32, spec: DeclId, main: &Locs) {
        let a = &img.attrs;
        let lo = a.insts.partition_point(|r| r.decl < inst);
        let mut ids = Vec::new();
        for r in a.insts[lo..].iter().take_while(|r| r.decl == inst) {
            let loc = main.get(r.loc);
            let syntax = syntax_of(r.syntax).unwrap_or_default();
            let node =
                AttrNode { kind: AttrKind(r.kind), syntax, flags: r.flags, loc, a: r.a, b: r.b };
            ids.push(self.ast.attrs.push(node));
            if r.name != NONE && loc.is_valid() {
                let scope = (r.scope != NONE).then(|| self.intern(&a.strs[r.scope as usize]));
                let name = self.intern(&a.strs[r.name as usize]);
                self.attr_spellings.insert(loc, (scope, name));
            }
        }
        if !ids.is_empty() {
            self.builder().append_attrs(spec, &ids);
        }
    }

    /// Fills in the expression arguments of the imported attributes.
    pub(super) fn image_attr_exprs(
        &mut self,
        img: &Image,
        pending: &AttrExprs,
        built: &super::body::Built,
    ) {
        let raw = |i: u32| if i == NONE { 0 } else { built.expr(i).index() };
        for &(id, a, b, keep, (start, len)) in pending {
            let b = if len != 0 {
                let run = &img.attrs.xexprs[start as usize..(start + len) as usize];
                let list: Vec<ExprId> = run.iter().map(|&x| built.expr(x)).collect();
                self.ast.lists.push(&list).raw()
            } else if b == NONE {
                keep
            } else {
                raw(b)
            };
            self.ast.attrs.set_args(id, raw(a), b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ANY, AttrKind, NOT_CARRIED, Op, op_of};

    const FLAG: Op = Op::Words(0, 0);
    /// The largest `zero_call_used_regs` kind index.
    const ZERO_CALL_KINDS: u32 = kiln_ast::attr::ZERO_CALL_USED_REGS_KINDS.len() as u32 - 1;
    /// The largest `#pragma clang section` kind index.
    const CLANG_SECTIONS: u32 = kiln_ast::attr::CLANG_SECTION_KINDS.len() as u32 - 1;

    /// The format before WP-2024 moved it into `kiln_ast::roles`: the
    /// kinds the image carries and how (`op_of` must derive exactly
    /// these, so the move changed no image). Flag-only kinds hold no
    /// argument words.
    const GOLDEN: &[(AttrKind, Op)] = &[
        (AttrKind::CARRIES_DEPENDENCY, FLAG),
        (AttrKind::DEPRECATED, Op::Exprs),
        (AttrKind::MAYBE_UNUSED, FLAG),
        (AttrKind::NODISCARD, Op::Exprs),
        (AttrKind::NORETURN, FLAG),
        (AttrKind::NO_UNIQUE_ADDRESS, FLAG),
        (AttrKind::ALWAYS_INLINE, FLAG),
        (AttrKind::NOINLINE, FLAG),
        (AttrKind::PACKED, FLAG),
        (AttrKind::USED, FLAG),
        (AttrKind::UNUSED, FLAG),
        (AttrKind::WEAK, FLAG),
        // 0 default, 1 hidden, 2 protected; `b` 1 for hidden written as
        // `internal` (an unchecked one, in a template, holds its argument
        // expression: not carried).
        (AttrKind::VISIBILITY, Op::Words(2, 1)),
        // The archetype, and the format and first data argument indices.
        (AttrKind::FORMAT, Op::Words(ANY, ANY)),
        (AttrKind::CONST, FLAG),
        (AttrKind::PURE, FLAG),
        (AttrKind::COLD, FLAG),
        (AttrKind::NOTHROW, FLAG),
        (AttrKind::TRIVIAL_ABI, FLAG),
        (AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION, FLAG),
        (AttrKind::ABI_TAG, Op::Sym(0)),
        (AttrKind::INTERNAL_LINKAGE, FLAG),
        // 1 for the implicit object parameter.
        (AttrKind::LIFETIMEBOUND, Op::Words(1, 0)),
        // The excluded sanitizer set and the spelling.
        (AttrKind::NO_SANITIZE, Op::Words(ANY, ANY)),
        (AttrKind::BUILTIN, Op::Builtin),
        (AttrKind::MALLOC, FLAG),
        // The one-based parameter indices, as integer literals.
        (AttrKind::ALLOC_SIZE, Op::Exprs),
        (AttrKind::ALLOC_ALIGN, Op::Exprs),
        (AttrKind::HOT, FLAG),
        (AttrKind::FLAG_ENUM, FLAG),
        (AttrKind::ENUM_EXTENSIBILITY, Op::Words(1, 0)),
        (AttrKind::OPTNONE, FLAG),
        (AttrKind::MINSIZE, FLAG),
        (AttrKind::NAKED, FLAG),
        (AttrKind::MAY_ALIAS, FLAG),
        (AttrKind::RETURNS_NONNULL, FLAG),
        (AttrKind::TYPE_VISIBILITY, Op::Words(2, 0)),
        (AttrKind::GNU_INLINE, FLAG),
        (AttrKind::NODEBUG, FLAG),
        (AttrKind::ANALYZER_NORETURN, FLAG),
        (AttrKind::DISABLE_TAIL_CALLS, FLAG),
        (AttrKind::ARTIFICIAL, FLAG),
        (AttrKind::RETURNS_TWICE, FLAG),
        (AttrKind::RETAIN, FLAG),
        (AttrKind::NO_DESTROY, FLAG),
        (AttrKind::ALWAYS_DESTROY, FLAG),
        (AttrKind::NO_STACK_PROTECTOR, FLAG),
        (AttrKind::NO_SPLIT_STACK, FLAG),
        (AttrKind::NO_INSTRUMENT_FUNCTION, FLAG),
        (AttrKind::NO_PROFILE_INSTRUMENT_FUNCTION, FLAG),
        (AttrKind::DISABLE_SANITIZER_INSTRUMENTATION, FLAG),
        (AttrKind::FLATTEN, FLAG),
        (AttrKind::LEAF, FLAG),
        (AttrKind::NOMERGE, FLAG),
        (AttrKind::NOT_TAIL_CALLED, FLAG),
        (AttrKind::NODUPLICATE, FLAG),
        (AttrKind::CONVERGENT, FLAG),
        (AttrKind::NOUWTABLE, FLAG),
        (AttrKind::NOESCAPE, FLAG),
        (AttrKind::LTO_VISIBILITY_PUBLIC, FLAG),
        (AttrKind::STANDALONE_DEBUG, FLAG),
        // Without a type argument (`b`, with `GSL_HAS_TYPE`): not carried.
        (AttrKind::GSL_OWNER, FLAG),
        (AttrKind::GSL_POINTER, FLAG),
        (AttrKind::UNAVAILABLE, Op::Exprs),
        (AttrKind::REQUIRE_CONSTANT_INIT, FLAG),
        (AttrKind::NONNULL, Op::Ints),
        // WP-1675: kept only for `__builtin_has_attribute`.
        (AttrKind::NONSTRING, FLAG),
        (AttrKind::NOCOMMON, FLAG),
        // WP-1310: the alignment expression (`__attribute__((aligned(E)))`,
        // `alignas(E)`), checked when it was written.
        (AttrKind::ALIGNED, Op::ExprA),
        (AttrKind::ASM_LABEL, Op::Label),
        // WP-1345: the typedef naming a specialization of the class template.
        (AttrKind::PREFERRED_NAME, Op::Type),
        // WP-1345: the condition and the message (string literal), both
        // expressions; the flags hold the diagnostic type, the group and
        // whether the condition names a parameter.
        (AttrKind::DIAGNOSE_IF, Op::Exprs),
        (AttrKind::ENABLE_IF, Op::Exprs),
        // WP-1806: the rest of the declaration attributes, so that no kind
        // is left out (`every_kind_is_carried_or_exempt`).
        (AttrKind::INDETERMINATE, FLAG),
        (AttrKind::SECTION, Op::Lit(0)),
        (AttrKind::USING_IF_EXISTS, FLAG),
        (AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE, FLAG),
        // The checked priority (101 to 65535).
        (AttrKind::INIT_PRIORITY, Op::Words(ANY, 0)),
        (AttrKind::UNINITIALIZED, FLAG),
        (AttrKind::TYPE_NULLABLE, FLAG),
        (AttrKind::TARGET, Op::Lit(0)),
        (AttrKind::MIN_VECTOR_WIDTH, Op::Words(ANY, 0)),
        // `a` zero once `weakref("target")` is split into an `alias`.
        (AttrKind::WEAKREF, Op::Lit(0)),
        (AttrKind::ALIAS, Op::LitLoc),
        (AttrKind::IFUNC, Op::LitLoc),
        // The checked priority; a dependent one (`PRIORITY_DEPENDENT`, its
        // expression in `a`) is not carried.
        (AttrKind::CONSTRUCTOR, Op::Words(ANY, 0)),
        (AttrKind::DESTRUCTOR, Op::Words(ANY, 0)),
        (AttrKind::CLEANUP, Op::DeclLoc),
        // The checked model, 1 to 4.
        (AttrKind::TLS_MODEL, Op::Words(4, 0)),
        // The object-size type and 1 for the dynamic spelling.
        (AttrKind::PASS_OBJECT_SIZE, Op::Words(3, 1)),
        (AttrKind::OVERLOADABLE, FLAG),
        (AttrKind::TRANSPARENT_UNION, FLAG),
        (AttrKind::SELECTANY, FLAG),
        (AttrKind::MS_STRUCT, FLAG),
        (AttrKind::GCC_STRUCT, FLAG),
        (AttrKind::NO_BUILTIN, Op::Sym(0)),
        // 1 for the `__clang_arm_builtin_alias` spelling.
        (AttrKind::BUILTIN_ALIAS, Op::Sym(1)),
        (AttrKind::TARGET_VERSION, Op::LitLoc),
        (AttrKind::TARGET_CLONES, Op::Sym(1)),
        (AttrKind::PATCHABLE_FUNCTION_ENTRY, Op::Words(ANY, ANY)),
        (AttrKind::ZERO_CALL_USED_REGS, Op::Words(ZERO_CALL_KINDS, 0)),
        (AttrKind::SPECULATIVE_LOAD_HARDENING, FLAG),
        (AttrKind::NO_SPECULATIVE_LOAD_HARDENING, FLAG),
        (AttrKind::XRAY_INSTRUMENT, Op::Words(1, 0)),
        (AttrKind::XRAY_LOG_ARGS, Op::Words(ANY, 0)),
        (AttrKind::NOCF_CHECK, FLAG),
        (AttrKind::FUNCTION_RETURN, Op::Words(1, 0)),
        (AttrKind::CFI_CANONICAL_JUMP_TABLE, FLAG),
        (AttrKind::FORCE_ALIGN_ARG_POINTER, FLAG),
        (AttrKind::ASSUME_ALIGNED, Op::Words(ANY, ANY)),
        (AttrKind::ALIGN_VALUE, Op::Words(ANY, 0)),
        (AttrKind::CALLBACK, Op::RawList),
        (AttrKind::MAYBE_UNDEF, FLAG),
        (AttrKind::LOADER_UNINITIALIZED, FLAG),
        (AttrKind::OMP_ASSUME, Op::Lit(0)),
        (AttrKind::CORO_AWAIT_ELIDABLE, FLAG),
        (AttrKind::CORO_AWAIT_ELIDABLE_ARGUMENT, FLAG),
        (AttrKind::ANNOTATE, Op::ExprList),
        // `b` the keyword's index and its state strings' offsets from `loc`
        // (`SME_KEYWORDS`), which the locations keep.
        (AttrKind::ARM_LOCALLY_STREAMING, Op::Words(0, ANY)),
        // Bit 0 ZA, bit 1 ZT0; `b` as above.
        (AttrKind::ARM_NEW, Op::Words(3, ANY)),
        (AttrKind::PRAGMA_CLANG_SECTION, Op::Lit(CLANG_SECTIONS)),
        // 1 `small`, 2 `large`.
        (AttrKind::CODE_MODEL, Op::Words(2, 0)),
        // `a` 1 (a failed signature check) is dropped before it is kept.
        (AttrKind::X86_INTERRUPT, FLAG),
        (AttrKind::WEAK_IMPORT, FLAG),
        // The `ParamAbi` (1 to 4) of a parameter.
        (AttrKind::SWIFT_PARAM_ABI, Op::Words(4, 0)),
    ];

    /// Every `AttrKind` constant of `kiln_ast::attr` (its name and
    /// number), read from the source so that a new kind is seen without
    /// a list to keep in step.
    fn all_kinds() -> Vec<(String, u16)> {
        let src = include_str!("../../../kiln_ast/src/attr.rs");
        let mut out = Vec::new();
        for line in src.lines() {
            let Some(rest) = line.trim().strip_prefix("pub const ") else { continue };
            let Some((name, rhs)) = rest.split_once(": AttrKind = AttrKind(") else { continue };
            let n = rhs.trim_end_matches(");").parse().expect("an AttrKind number");
            out.push((name.to_owned(), n));
        }
        out
    }

    /// WP-1806: each kind is carried by the table, consumed into a type
    /// (which the type table carries), or listed in `NOT_CARRIED` with
    /// the reason no declaration the image writes holds it; never two of
    /// these. A kind added to `kiln_ast::attr` without a row fails here
    /// instead of making every declaration with it fall back unnoticed.
    #[test]
    fn every_kind_is_carried_or_exempt() {
        let kinds = all_kinds();
        assert!(kinds.len() > 140, "the AttrKind constants were not found");
        let mut missing = Vec::new();
        for (name, n) in &kinds {
            let k = AttrKind(*n);
            let carried = usize::from(op_of(k).is_some());
            let exempt = NOT_CARRIED.iter().filter(|(t, _)| *t == k).count();
            let ty = usize::from(k.is_vector_type() || k.is_fn_type_attr());
            match carried + exempt + ty {
                1 => {}
                0 => missing.push(name.clone()),
                _ => panic!(
                    "AttrKind::{name} is in more than one of the carried kinds, NOT_CARRIED and \
                     the type attributes"
                ),
            }
        }
        assert!(
            missing.is_empty(),
            "AttrKind constants whose kiln_ast::roles row gives no image Op (or with no \
             NOT_CARRIED reason): {missing:?}"
        );
        for (k, _) in NOT_CARRIED {
            assert!(kinds.iter().any(|(_, n)| *n == k.0), "a row for unknown kind {}", k.0);
        }
    }

    /// WP-2024: the ops derived from `kiln_ast::roles` are the format the
    /// image had when it kept its own table: the same kinds, each written
    /// the same way.
    #[test]
    fn derived_ops_are_the_golden_table() {
        let mut diff = Vec::new();
        for (name, n) in all_kinds() {
            let k = AttrKind(n);
            let want = GOLDEN.iter().find(|&&(t, _)| t == k).map(|&(_, op)| op);
            if op_of(k) != want {
                diff.push(format!("{name}: derived {:?}, golden {want:?}", op_of(k)));
            }
        }
        assert!(diff.is_empty(), "{diff:#?}");
        assert!(GOLDEN.iter().any(|&(_, op)| op == Op::Words(ANY, ANY)));
    }
}
