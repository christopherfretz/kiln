//! The canonical C type of a C-linkage entity, for the declaration rows
//! of the link-time metadata (`.mason.meta` record kind 5; mortar
//! `spec/odr.md` O-33; spec/codegen.md "Link-time metadata
//! (`.mason.meta`)", "The canonical C type", WP-1564).
//!
//! A C unit and a C++ unit that declare one C function must produce the
//! same bytes, so the form erases every difference that the C and the
//! C++ spelling of one declaration can legitimately have: typedefs are
//! resolved, every record type is one opaque token `R`, the target's
//! `va_list` is `V`, an enumeration is its integer type, C++'s character
//! and `bool` types are C's, references are pointers, `noexcept` and
//! `restrict` are dropped, top-level qualifiers of parameters and of the
//! return type are dropped, array bounds are erased, and a
//! `transparent_union` parameter is its first member's type. A type with
//! no C form (a member pointer, a non-default calling convention, an
//! unprototyped function type inside a type) has no canonical form:
//! [`function`] and [`object`] return `None` and the entity gets no row.
//!
//! The grammar (every token; the spec repeats it, a second producer must
//! reproduce it byte for byte):
//!
//! ```text
//! <type>    ::= [K] [Q] <core>          const, volatile (restrict never)
//! <core>    ::= <builtin>
//!             | R                       any struct, union or class
//!             | V                       the target's va_list
//!             | P <type>                pointer, lvalue or rvalue reference
//!             | A <type>                array of any or unknown bound, VLA
//!             | F <type> <params> E     prototyped function type
//!             | U <type>                _Atomic
//!             | C <builtin>             _Complex
//!             | Dv <lanes> _ <type>     vector
//!             | DB <bits> _ | DU <bits> _   _BitInt(N), unsigned _BitInt(N)
//!             | Dn                      nullptr_t
//!             | u <len> <name>          a sizeless vendor type (SVE)
//! <params>  ::= v | <type>+ | <type>+ z | z
//! <builtin> ::= v b c a h s t i j l m x y n o | f d e g Dh Db
//! ```
//!
//! Builtins: `v` void, `b` `_Bool`/`bool`, `c` char, `a` signed char, `h`
//! unsigned char and `char8_t`, `s` short, `t` unsigned short and
//! `char16_t`, `i` int, `j` unsigned int and `char32_t`, `l` long, `m`
//! unsigned long, `x` long long, `y` unsigned long long, `n` `__int128`,
//! `o` unsigned `__int128`; `wchar_t` is the target's `wchar_t` typedef
//! type (`i` or `j`). Floating types by format: `Dh` binary16 (`_Float16`,
//! `__fp16`), `Db` bfloat16, `f` binary32 (`float`, `_Float32`), `d`
//! binary64 (`double`, `_Float64`, `_Float32x`), `e` x87 80-bit, `g`
//! binary128 (`__float128`, `_Float128`; `long double` and `_Float64x` are
//! `e` or `g` by the target's format).

use kiln_abi::Target;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::types::{BuiltinType, CallConv, FloatFormat, TypeView};
use kiln_ast::{Ast, DeclId, QualType};

/// The deepest type the form follows (a degenerate type gets no row).
const MAX_DEPTH: u32 = 64;

/// The canonical form of function type `fty`, or `None` when it has none
/// (an unprototyped function, or a type with no C form inside).
pub(crate) fn function(ast: &Ast, target: &Target, fty: QualType) -> Option<String> {
    let mut w = Writer { ast, target, out: String::new() };
    w.function(fty, 0)?;
    Some(w.out)
}

/// The canonical form of object type `q` (a variable's declared type;
/// its own qualifiers kept), or `None`.
pub(crate) fn object(ast: &Ast, target: &Target, q: QualType) -> Option<String> {
    let mut w = Writer { ast, target, out: String::new() };
    w.ty(q, true, 0)?;
    Some(w.out)
}

struct Writer<'a> {
    ast: &'a Ast,
    target: &'a Target,
    out: String,
}

impl Writer<'_> {
    /// `F <ret> <params> E`: the return type and every parameter without
    /// their top-level qualifiers, a `transparent_union` parameter as its
    /// first member's type.
    fn function(&mut self, fty: QualType, depth: u32) -> Option<()> {
        let ast = self.ast;
        let p = *ast.types.proto(ast.types.canonical(fty))?;
        let info = p.info();
        let plain_cc = matches!(info.cc(), CallConv::Default | CallConv::C);
        if info.no_proto()
            || !plain_cc
            || info.regparm().is_some()
            || info.no_caller_saved_regs()
            || info.sme() != 0
        {
            return None;
        }
        self.out.push('F');
        self.ty(p.ret(), false, depth + 1)?;
        let params = ast.types.params(&p);
        if params.is_empty() && !p.variadic() {
            self.out.push('v');
        }
        for &q in params {
            let q = kiln_abi::call::transparent_union_first_field(ast, q).unwrap_or(q);
            self.ty(q, false, depth + 1)?;
        }
        if p.variadic() {
            self.out.push('z');
        }
        self.out.push('E');
        Some(())
    }

    /// `q`, with its own `const`/`volatile` when `quals` (pointees,
    /// array elements and variables), without them otherwise (parameters
    /// and return types).
    fn ty(&mut self, q: QualType, quals: bool, depth: u32) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let ast = self.ast;
        let q = ast.types.canonical(q);
        let view = ast.types.canonical_view(q);
        // An array carries its element's qualifiers: they are written on
        // the element, after `A`.
        let is_array = matches!(
            view,
            TypeView::ConstantArray(_)
                | TypeView::IncompleteArray(_)
                | TypeView::VariableArray { .. }
                | TypeView::DependentSizedArray { .. }
        );
        if quals && !is_array {
            let qs = ast.types.effective_quals(q);
            if qs.is_const() {
                self.out.push('K');
            }
            if qs.is_volatile() {
                self.out.push('Q');
            }
        }
        let d = depth + 1;
        match view {
            TypeView::Builtin(b) => self.builtin(b)?,
            TypeView::BitInt { signed, width } => {
                self.out.push_str(if signed { "DB" } else { "DU" });
                self.out.push_str(&width.to_string());
                self.out.push('_');
            }
            TypeView::Enum(e) => {
                let int = enum_int(ast, e)?;
                self.ty(int, false, d)?;
            }
            TypeView::Record(r) => {
                self.out.push(if is_va_list_record(ast, r) { 'V' } else { 'R' });
            }
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                // x86-64's `va_list` decays to `__va_list_tag *`.
                if let TypeView::Record(r) = ast.types.canonical_view(p)
                    && is_va_list_record(ast, r)
                    && self.target.triple.arch == kiln_abi::Arch::X86_64
                {
                    self.out.push('V');
                } else {
                    self.out.push('P');
                    self.ty(p, true, d)?;
                }
            }
            TypeView::ConstantArray(_)
            | TypeView::IncompleteArray(_)
            | TypeView::VariableArray { .. }
            | TypeView::DependentSizedArray { .. } => {
                let elem = ast.types.element(q)?;
                // x86-64's `va_list` is `__va_list_tag[1]`.
                if let TypeView::Record(r) = ast.types.canonical_view(elem)
                    && is_va_list_record(ast, r)
                {
                    self.out.push('V');
                } else {
                    self.out.push('A');
                    self.ty(elem, true, d)?;
                }
            }
            TypeView::Function(_) => self.function(q, d)?,
            TypeView::Atomic(e) => {
                self.out.push('U');
                self.ty(e, true, d)?;
            }
            TypeView::Complex(e) => {
                self.out.push('C');
                self.ty(e, false, d)?;
            }
            TypeView::Vector { elem, count, .. } => {
                self.out.push_str("Dv");
                self.out.push_str(&count.to_string());
                self.out.push('_');
                self.ty(elem, true, d)?;
            }
            _ => return None,
        }
        Some(())
    }

    fn builtin(&mut self, b: BuiltinType) -> Option<()> {
        use BuiltinType as B;
        let ld = self.target.types.long_double;
        let s = match b {
            B::Void => "v",
            B::Bool => "b",
            B::Char => "c",
            B::SChar => "a",
            B::UChar | B::Char8 => "h",
            B::Short => "s",
            B::UShort | B::Char16 => "t",
            B::Int => "i",
            B::UInt | B::Char32 => "j",
            B::WChar => {
                if self.target.types.wchar_signed {
                    "i"
                } else {
                    "j"
                }
            }
            B::Long => "l",
            B::ULong => "m",
            B::LongLong => "x",
            B::ULongLong => "y",
            B::Int128 => "n",
            B::UInt128 => "o",
            B::Float16 | B::Half => "Dh",
            B::BFloat16 => "Db",
            B::Float | B::Float32 => "f",
            B::Double | B::Float64 | B::Float32x => "d",
            B::Float128 | B::StdFloat128 => "g",
            B::LongDouble | B::Float64x => match ld {
                FloatFormat::X87Extended => "e",
                FloatFormat::IeeeQuad => "g",
                FloatFormat::IeeeDouble => "d",
                _ => return None,
            },
            B::NullPtr => "Dn",
            _ if b.is_sizeless() => {
                let name = b.name();
                self.out.push('u');
                self.out.push_str(&name.len().to_string());
                self.out.push_str(name);
                return Some(());
            }
            _ => return None,
        };
        self.out.push_str(s);
        Some(())
    }
}

/// The integer type kiln chose for enumeration `e` (its fixed underlying
/// type, or the one sema computed from the enumerators, `-fshort-enums`
/// included); `None` while it has none (an incomplete enumeration).
fn enum_int(ast: &Ast, e: DeclId) -> Option<QualType> {
    ast.decls.enumeration(ast.definition(e).unwrap_or(e)).and_then(|x| x.int_type)
}

/// `r` is the record behind the target's `__builtin_va_list` (the
/// implicit `std::__va_list` of `AArch64` or `__va_list_tag` of x86-64,
/// `kiln_sema` `declare_builtin_va_list`).
fn is_va_list_record(ast: &Ast, r: DeclId) -> bool {
    let n = ast.decls.get(r);
    if !n.has(DeclFlags::IMPLICIT) || n.kind() != DeclKind::CXXRecord {
        return false;
    }
    n.name()
        .as_ident()
        .is_some_and(|s| matches!(ast.symbols().as_str(s), "__va_list" | "__va_list_tag"))
}
