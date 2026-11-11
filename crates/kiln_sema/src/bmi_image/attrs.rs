//! Declaration attributes and builtin identity in the declaration image
//! (spec/modules.md §11.11, "Attributes", WP-1278).
//!
//! Every attribute node on a carried declaration is one [`AttrRec`]: the
//! node's kind, syntax, flags and location, and its two argument words
//! written as [`TABLE`] says for the kind. The table is the whole format:
//! a kind it does not list, or a node whose words fall outside what its
//! row allows, makes the declaration fall back
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
use kiln_ast::{AttrId, DeclId, ExprId};
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
    /// `a` is a `Symbol` (written as its text), `b` zero.
    Sym,
    /// `a` and `b` are each an expression or zero.
    Exprs,
    /// `a` is a builtin id (written with its spelling), `b` zero.
    Builtin,
}

const ANY: u32 = u32::MAX;
const FLAG: Op = Op::Words(0, 0);

/// The kinds the image carries. Flag-only kinds hold no argument words;
/// the rest name what their words are (see `kiln_ast::attr`).
const TABLE: &[(AttrKind, Op)] = &[
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
    // 0 default, 1 hidden, 2 protected (an unchecked one, in a template,
    // holds its argument expression: not carried).
    (AttrKind::VISIBILITY, Op::Words(2, 0)),
    // The archetype, and the format and first data argument indices.
    (AttrKind::FORMAT, Op::Words(ANY, ANY)),
    (AttrKind::CONST, FLAG),
    (AttrKind::PURE, FLAG),
    (AttrKind::COLD, FLAG),
    (AttrKind::NOTHROW, FLAG),
    (AttrKind::TRIVIAL_ABI, FLAG),
    (AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION, FLAG),
    (AttrKind::ABI_TAG, Op::Sym),
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
];

fn op_of(k: AttrKind) -> Option<Op> {
    TABLE.iter().find(|&&(t, _)| t == k).map(|&(_, op)| op)
}

/// Whether the image carries `node` (its kind and argument words).
pub(super) fn carried(node: &AttrNode, nsyms: usize) -> bool {
    match op_of(node.kind) {
        None => false,
        Some(Op::Words(am, bm)) => node.a <= am && node.b <= bm,
        Some(Op::Sym) => (node.a as usize) < nsyms && node.b == 0,
        Some(Op::Exprs) => true,
        Some(Op::Builtin) => node.b == 0 && builtin::info(node.a).is_some(),
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

/// One attribute node of record `decl`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct AttrRec {
    pub(super) decl: u32,
    kind: u16,
    syntax: u8,
    flags: u8,
    loc: u32,
    /// The words; for [`Op::Exprs`] expression record indices or
    /// [`NONE`].
    pub(super) a: u32,
    pub(super) b: u32,
    /// [`Op::Sym`]: the text; [`Op::Builtin`]: the builtin's spelling;
    /// else [`NONE`] (an index into [`Attrs::strs`]).
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
}

/// What the writer computed for one declaration, committed with its
/// record ([`Writer::attr_commit`]).
#[derive(Default)]
pub(super) struct Pending {
    recs: Vec<(AttrRec, [Option<ExprId>; 2])>,
    builtins: Vec<BuiltinRec>,
}

impl Writer<'_, '_, '_> {
    /// Whether every attribute of `d` is one the image carries.
    pub(super) fn attrs_carried(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.symbols().len();
        ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).all(|a| carried(ast.attrs.get(a), n))
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
            let (scope, name) = match s.attr_spellings.get(&node.loc) {
                Some(&(sc, nm)) if node.loc.is_valid() => {
                    let sc = sc.map(|x| ast.symbols().as_str(x).to_owned());
                    let nm = ast.symbols().as_str(nm).to_owned();
                    (sc.map_or(NONE, |x| self.attr_str(&x)), self.attr_str(&nm))
                }
                _ => (NONE, NONE),
            };
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
            match op {
                Op::Words(..) => {}
                Op::Sym => {
                    let t = ast.symbols().as_str(Symbol::from_index(node.a)).to_owned();
                    r.s = self.attr_str(&t);
                    r.a = 0;
                }
                Op::Exprs => {
                    exprs = [ExprId::try_new(node.a), ExprId::try_new(node.b)];
                    (r.a, r.b) = (NONE, NONE);
                }
                Op::Builtin => {
                    let info = builtin::info(node.a).ok_or(UNCARRIED)?;
                    r.s = self.attr_str(info.name);
                }
            }
            p.recs.push((r, exprs));
        }
        Ok(p)
    }

    /// Adds `p` to the image, once its declaration's record is in.
    pub(super) fn attr_commit(&mut self, rec: u32, p: Pending) {
        for (r, exprs) in p.recs {
            let k = self.img.attrs.recs.len() as u32;
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
        w.u32(r.decl);
        w.u16(r.kind);
        w.u8(r.syntax);
        w.u8(r.flags);
        for x in [r.loc, r.a, r.b, r.s, r.scope, r.name] {
            w.u32(x);
        }
    }
    w.len(a.builtins.len());
    for b in &a.builtins {
        w.u32(b.decl);
        w.u8(b.map);
        w.u32(b.id);
        w.u32(b.name);
    }
}

pub(super) fn decode_attrs(r: &mut Rd<'_>) -> Res<Attrs> {
    let mut a = Attrs::default();
    let n = r.count(4)?;
    for _ in 0..n {
        a.strs.push(r.str()?);
    }
    let n = r.count(32)?;
    for _ in 0..n {
        let decl = r.u32()?;
        let kind = r.u16()?;
        let syntax = r.u8()?;
        let flags = r.u8()?;
        let [loc, x, y, s, scope, name] =
            [r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        a.recs.push(AttrRec { decl, kind, syntax, flags, loc, a: x, b: y, s, scope, name });
    }
    let n = r.count(13)?;
    for _ in 0..n {
        let decl = r.u32()?;
        let map = r.u8()?;
        let id = r.u32()?;
        let name = r.u32()?;
        a.builtins.push(BuiltinRec { decl, map, id, name });
    }
    Ok(a)
}

/// Checks every index of the attribute section.
pub(super) fn validate_attrs(img: &Image) -> Res<()> {
    let a = &img.attrs;
    let str_ok = |i: u32| (i as usize) < a.strs.len();
    let spelled = |i: u32, id: u32| {
        str_ok(i) && builtin::info(id).is_some_and(|b| b.name == a.strs[i as usize])
    };
    let nexpr = img.bodies.exprs.len();
    let expr_ok = |i: u32| i == NONE || (i as usize) < nexpr;
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
                Some(Op::Sym) => r.a == 0 && r.b == 0 && str_ok(r.s),
                Some(Op::Exprs) => expr_ok(r.a) && expr_ok(r.b) && r.s == NONE,
                Some(Op::Builtin) => r.b == 0 && spelled(r.s, r.a),
            };
        if !ok {
            return bad("bad attribute record");
        }
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
/// built: (attribute, `a` record, `b` record).
pub(super) type AttrExprs = Vec<(AttrId, u32, u32)>;

impl Sema<'_, '_> {
    /// Attaches record `rec`'s attributes and builtin identity to `d`,
    /// noting the expression arguments in `pending`.
    pub(super) fn image_attrs(
        &mut self,
        img: &Image,
        rec: u32,
        d: DeclId,
        main: &Locs,
        pending: &mut AttrExprs,
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
            let (x, y) = match op_of(kind) {
                Some(Op::Sym) => (self.intern(&a.strs[r.s as usize]).index(), 0),
                Some(Op::Exprs) => (0, 0),
                _ => (r.a, r.b),
            };
            let id = self.ast.attrs.push(AttrNode { kind, syntax, flags: r.flags, loc, a: x, b: y });
            if op_of(kind) == Some(Op::Exprs) && (r.a != NONE || r.b != NONE) {
                pending.push((id, r.a, r.b));
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

    /// Fills in the expression arguments of the imported attributes.
    pub(super) fn image_attr_exprs(&mut self, pending: &AttrExprs, built: &super::body::Built) {
        let raw = |i: u32| if i == NONE { 0 } else { built.expr(i).index() };
        for &(id, a, b) in pending {
            self.ast.attrs.set_args(id, raw(a), raw(b));
        }
    }
}
