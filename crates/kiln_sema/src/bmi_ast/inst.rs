//! Keys for evaluations inside template instantiations (spec/modules.md
//! §11.11 "Instantiation keys", WP-1046).
//!
//! An evaluation inside an instantiation is keyed by the instantiated
//! entity of the innermost instantiation frame, written as a byte string
//! that names no `DeclId`: the template and its canonical arguments for a
//! specialization, the enclosing instantiated entity and the pattern
//! member for a member of a specialization (or any other instantiated
//! declaration), and kind, name and replay-stable position for a
//! declaration that was not instantiated. Types are written structurally
//! (canonical, with their cv bits); values as part 1's carried values.
//! The producing compile and the importer's replay build the same bytes
//! for the same specialization, so the bytes are compared as they are.
//! A key that cannot be formed (a dependent or unsupported type, a
//! pointer or reflection argument, an implicit declaration, a position
//! the replay does not reproduce) leaves the evaluation uncarried.

use kiln_ast::decl::DeclFlags;
use kiln_ast::ids::ArgListId;
use kiln_ast::names::DeclNameKind;
use kiln_ast::types::{ArgView, ExcSpec, TemplateName, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::{FileId, SourceManager};

use super::{Rd, Res, bad, carried_value, loc_key, put_bytes, put_u32, put_value, read_value};

/// The deepest nesting a key may have (entities, argument lists and
/// types together); deeper keys are not formed.
const MAX_DEPTH: u32 = 48;

/// The longest key formed, in bytes.
const MAX_KEY: usize = 1 << 14;

/// Entity tags.
const ENT_SPEC: u8 = b'S';
const ENT_MEMBER: u8 = b'M';
const ENT_PATTERN: u8 = b'P';
const ENT_DECL: u8 = b'D';
/// Top level only: a substitution frame ([`subst_key`]).
const ENT_SUBST: u8 = b'F';

/// Argument tags.
const ARG_TYPE: u8 = b'T';
const ARG_VALUE: u8 = b'V';
const ARG_TEMPLATE: u8 = b'N';
const ARG_PACK: u8 = b'K';

/// Type tags (after the cv byte).
const TY_BUILTIN: u8 = 0;
const TY_BITINT: u8 = 1;
const TY_POINTER: u8 = 2;
const TY_LREF: u8 = 3;
const TY_RREF: u8 = 4;
const TY_MEMBER_POINTER: u8 = 5;
const TY_ARRAY: u8 = 6;
const TY_INCOMPLETE_ARRAY: u8 = 7;
const TY_FUNCTION: u8 = 8;
const TY_RECORD: u8 = 9;
const TY_ENUM: u8 = 10;
const TY_COMPLEX: u8 = 11;
const TY_ATOMIC: u8 = 12;
const TY_VECTOR: u8 = 13;

/// The writer of one key. `file` maps a file to its index in the
/// section's path table (`None`: a file the replay does not reproduce).
struct KeyCx<'a, F> {
    ast: &'a Ast,
    sm: &'a SourceManager,
    file: F,
    out: Vec<u8>,
}

/// The key of instantiated entity `d`, or `None` when it cannot be
/// formed or `d` is not an instantiated entity (its key would not name
/// the arguments the evaluation was substituted with).
pub(super) fn entity_key(
    ast: &Ast,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    d: DeclId,
) -> Option<Box<[u8]>> {
    let mut cx = KeyCx { ast, sm, file, out: Vec::new() };
    cx.ent(d, 0)?;
    (cx.out[0] != ENT_DECL).then(|| cx.out.into_boxed_slice())
}

/// The key of a substitution frame of kind `code` into `template` with
/// the innermost arguments `args`, at `entity` (the template itself, or
/// the parameter whose type or default argument is substituted). Only a
/// template whose parameters are its outermost level (depth 0) is keyed:
/// then `args` are all the arguments the substitution uses, and an
/// enclosing specialization's are in the template's own key.
pub(super) fn subst_key(
    ast: &Ast,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    code: u8,
    template: DeclId,
    entity: DeclId,
    args: ArgListId,
) -> Option<Box<[u8]>> {
    let info = ast.template_info(template)?;
    if ast.templates.param_list(info.params).depth != 0 {
        return None;
    }
    let mut cx = KeyCx { ast, sm, file, out: vec![ENT_SUBST, code] };
    cx.ent(template, 1)?;
    cx.ent(entity, 1)?;
    cx.args(ast.types.canonical_args(args), 1)?;
    Some(cx.out.into_boxed_slice())
}

impl<F: FnMut(FileId) -> Option<u32>> KeyCx<'_, F> {
    fn room(&self, depth: u32) -> Option<()> {
        (depth <= MAX_DEPTH && self.out.len() <= MAX_KEY).then_some(())
    }

    fn ent(&mut self, d: DeclId, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let d = ast.first(d);
        if let Some((_, s)) = ast.spec_info(d) {
            let (template, args) = (s.template, s.args);
            self.out.push(ENT_SPEC);
            self.ent(template, depth + 1)?;
            return self.args(ast.types.canonical_args(args), depth + 1);
        }
        let inst = ast
            .member_spec_info(d)
            .map(|(_, m)| (ENT_MEMBER, m.instantiated_from))
            .or_else(|| ast.templates.instantiated_from_map(d).map(|p| (ENT_PATTERN, p)));
        if let Some((tag, pattern)) = inst {
            let ctx = ast.decls.get(d).ctx()?;
            self.out.push(tag);
            self.ent(ctx, depth + 1)?;
            return self.ent(pattern, depth + 1);
        }
        // A declaration that was not instantiated, inside nothing that
        // was: its position identifies it. An instantiated declaration
        // without a link to its pattern would share its pattern's
        // position with every other instantiation, so it has no key.
        let n = ast.decls.get(d);
        if n.has(DeclFlags::IMPLICIT) {
            return None;
        }
        let mut c = n.ctx();
        for _ in 0..256 {
            let Some(x) = c else { break };
            let x = ast.first(x);
            if ast.spec_info(x).is_some()
                || ast.member_spec_info(x).is_some()
                || ast.templates.instantiated_from_map(x).is_some()
            {
                return None;
            }
            c = ast.decls.get(x).ctx();
        }
        if c.is_some() {
            return None;
        }
        self.out.push(ENT_DECL);
        self.out.push(n.kind() as u8);
        let name = n.name();
        self.out.push(name.kind() as u8);
        match name.kind() {
            DeclNameKind::Identifier => {
                let s = name.as_ident().map_or("", |s| ast.symbols().as_str(s));
                put_bytes(&mut self.out, s.as_bytes());
            }
            DeclNameKind::Operator => self.out.push(name.payload() as u8),
            _ => {}
        }
        let at = loc_key(self.sm, n.loc(), &mut self.file)?;
        for w in at.0 {
            put_u32(&mut self.out, w);
        }
        Some(())
    }

    fn args(&mut self, l: ArgListId, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let args = ast.types.args(l);
        put_u32(&mut self.out, u32::try_from(args.len()).ok()?);
        for a in args {
            match a.view() {
                ArgView::Type(q) => {
                    self.out.push(ARG_TYPE);
                    self.ty(q, depth + 1)?;
                }
                ArgView::Value { ty, value } => {
                    self.out.push(ARG_VALUE);
                    self.ty(ty, depth + 1)?;
                    let v = carried_value(&ast.consts, value).ok()?;
                    put_value(&mut self.out, &v);
                }
                ArgView::Template(n) => {
                    let n = ast.types.canonical_template_name(n);
                    let TemplateName::Template(d) = ast.types.template_name_view(n) else {
                        return None;
                    };
                    self.out.push(ARG_TEMPLATE);
                    self.ent(d, depth + 1)?;
                }
                ArgView::Pack(p) => {
                    self.out.push(ARG_PACK);
                    self.args(p, depth + 1)?;
                }
                _ => return None,
            }
        }
        Some(())
    }

    fn ty(&mut self, q: QualType, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let types = &ast.types;
        let c = types.canonical(q);
        self.out.push(c.quals_bits());
        let d = depth + 1;
        match types.view(c.ty()) {
            TypeView::Builtin(b) => self.out.extend_from_slice(&[TY_BUILTIN, b as u8]),
            TypeView::BitInt { signed, width } => {
                self.out.extend_from_slice(&[TY_BITINT, u8::from(signed)]);
                put_u32(&mut self.out, width);
            }
            TypeView::Pointer(p) => self.wrap(TY_POINTER, p, d)?,
            TypeView::LValueRef(p) => self.wrap(TY_LREF, p, d)?,
            TypeView::RValueRef(p) => self.wrap(TY_RREF, p, d)?,
            TypeView::MemberPointer { pointee, class } => {
                self.wrap(TY_MEMBER_POINTER, pointee, d)?;
                self.ty(QualType::unqual(class), d)?;
            }
            TypeView::ConstantArray(a) => {
                self.out.push(TY_ARRAY);
                self.out.extend_from_slice(&a.size.to_le_bytes());
                self.ty(a.elem, d)?;
            }
            TypeView::IncompleteArray(e) => self.wrap(TY_INCOMPLETE_ARRAY, e, d)?,
            TypeView::Function(p) => {
                let exc = match types.exc_spec(&p) {
                    ExcSpec::None => 0,
                    ExcSpec::Nothrow => 1,
                    _ => return None,
                };
                self.wrap(TY_FUNCTION, p.ret(), d)?;
                let params = types.params(&p);
                put_u32(&mut self.out, params.len() as u32);
                for &t in params {
                    self.ty(t, d)?;
                }
                self.out.extend_from_slice(&p.info().bits().to_le_bytes());
                self.out.push(exc);
            }
            TypeView::Record(r) => {
                self.out.push(TY_RECORD);
                self.ent(r, d)?;
            }
            TypeView::Enum(e) => {
                self.out.push(TY_ENUM);
                self.ent(e, d)?;
            }
            TypeView::Complex(e) => self.wrap(TY_COMPLEX, e, d)?,
            TypeView::Atomic(e) => self.wrap(TY_ATOMIC, e, d)?,
            TypeView::Vector { elem, count, kind } => {
                self.wrap(TY_VECTOR, elem, d)?;
                put_u32(&mut self.out, count);
                self.out.push(kind as u8);
            }
            _ => return None,
        }
        Some(())
    }

    fn wrap(&mut self, tag: u8, inner: QualType, depth: u32) -> Option<()> {
        self.out.push(tag);
        self.ty(inner, depth)
    }
}

// --- checking a decoded key --------------------------------------------

/// Checks that `b` is a well-formed key whose path indices are below
/// `npaths`. Keys are only compared with the importer's own, never
/// interpreted, but the reader still rejects malformed ones (the section
/// is damaged).
pub(super) fn check_key(b: &[u8], npaths: usize) -> Res<()> {
    let mut ck = Ck { r: Rd(b), npaths };
    match b.first() {
        Some(&ENT_SPEC | &ENT_MEMBER | &ENT_PATTERN) => ck.ent(0)?,
        Some(&ENT_SUBST) => {
            ck.r.take(2)?;
            ck.ent(1)?;
            ck.ent(1)?;
            ck.args(1)?;
        }
        _ => return bad("bad instantiation key"),
    }
    if !ck.r.0.is_empty() {
        return bad("bad instantiation key");
    }
    Ok(())
}

struct Ck<'a> {
    r: Rd<'a>,
    npaths: usize,
}

impl Ck<'_> {
    fn depth(depth: u32) -> Res<()> {
        if depth > MAX_DEPTH + 1 { bad("instantiation key nested too deeply") } else { Ok(()) }
    }

    fn ent(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        match self.r.u8()? {
            ENT_SPEC => {
                self.ent(depth + 1)?;
                self.args(depth + 1)
            }
            ENT_MEMBER | ENT_PATTERN => {
                self.ent(depth + 1)?;
                self.ent(depth + 1)
            }
            ENT_DECL => {
                let _kind = self.r.u8()?;
                match self.r.u8()? {
                    0 => {
                        self.r.bytes()?;
                    }
                    1 => {
                        self.r.u8()?;
                    }
                    2..=6 => {}
                    _ => return bad("bad instantiation key"),
                }
                for i in 0..4 {
                    let w = self.r.u32()?;
                    if i % 2 == 0 && w as usize >= self.npaths {
                        return bad("path index out of range");
                    }
                }
                Ok(())
            }
            _ => bad("bad instantiation key"),
        }
    }

    fn args(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        let n = self.r.count(2)?;
        for _ in 0..n {
            match self.r.u8()? {
                ARG_TYPE => self.ty(depth + 1)?,
                ARG_VALUE => {
                    self.ty(depth + 1)?;
                    read_value(&mut self.r)?;
                }
                ARG_TEMPLATE => self.ent(depth + 1)?,
                ARG_PACK => self.args(depth + 1)?,
                _ => return bad("bad instantiation key"),
            }
        }
        Ok(())
    }

    fn ty(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        if self.r.u8()? > 7 {
            return bad("bad instantiation key");
        }
        let d = depth + 1;
        match self.r.u8()? {
            TY_BUILTIN => {
                self.r.u8()?;
            }
            TY_BITINT => {
                self.r.flag()?;
                self.r.u32()?;
            }
            TY_POINTER | TY_LREF | TY_RREF | TY_INCOMPLETE_ARRAY | TY_COMPLEX | TY_ATOMIC => {
                self.ty(d)?;
            }
            TY_MEMBER_POINTER => {
                self.ty(d)?;
                self.ty(d)?;
            }
            TY_ARRAY => {
                self.r.u64()?;
                self.ty(d)?;
            }
            TY_FUNCTION => {
                self.ty(d)?;
                let n = self.r.count(2)?;
                for _ in 0..n {
                    self.ty(d)?;
                }
                self.r.take(2)?;
                if self.r.u8()? > 1 {
                    return bad("bad instantiation key");
                }
            }
            TY_RECORD | TY_ENUM => self.ent(d)?,
            TY_VECTOR => {
                self.ty(d)?;
                self.r.u32()?;
                self.r.u8()?;
            }
            _ => return bad("bad instantiation key"),
        }
        Ok(())
    }
}
