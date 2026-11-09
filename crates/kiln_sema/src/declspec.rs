//! Declaration specifiers and declarators as the parser collects them
//! (clang's `DeclSpec`/`Declarator`, spec/sema.md §4).
//!
//! The parser records what was written; [`crate::Sema`] turns a
//! [`DeclSpec`] into a type ([dcl.type.simple]) and a [`Declarator`] into
//! a declaration.

use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, Quals, RefQual, TagKeyword, TypeAttrKind};
use kiln_ast::{AttrId, DeclId, ExprId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

/// A storage-class specifier ([dcl.stc]), or `typedef` (which clang
/// keeps in the same slot).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StorageSpec {
    /// `typedef`.
    Typedef,
    /// `static`.
    Static,
    /// `extern`.
    Extern,
    /// `register` (ill-formed in C++17).
    Register,
    /// `mutable`.
    Mutable,
}

impl StorageSpec {
    /// The keyword.
    pub fn spelling(self) -> &'static str {
        match self {
            StorageSpec::Typedef => "typedef",
            StorageSpec::Static => "static",
            StorageSpec::Extern => "extern",
            StorageSpec::Register => "register",
            StorageSpec::Mutable => "mutable",
        }
    }
}

/// `short`/`long`/`long long`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Width {
    /// None written.
    #[default]
    None,
    /// `short`.
    Short,
    /// `long`.
    Long,
    /// `long long`.
    LongLong,
}

/// `signed`/`unsigned`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Sign {
    /// None written.
    #[default]
    None,
    /// `signed`.
    Signed,
    /// `unsigned`.
    Unsigned,
}

/// The base type-specifier ([dcl.type.simple]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum TypeSpecKind {
    /// None written.
    #[default]
    None,
    /// `void`.
    Void,
    /// `bool`.
    Bool,
    /// `char`.
    Char,
    /// `wchar_t`.
    WChar,
    /// `char8_t`.
    Char8,
    /// `char16_t`.
    Char16,
    /// `char32_t`.
    Char32,
    /// `int`.
    Int,
    /// `__int128`.
    Int128,
    /// `_BitInt ( constant-expression )` (C23 bit-precise integer, a
    /// clang extension in C++; spec/types.md §21): the width is in
    /// [`DeclSpec::bit_int_width`].
    BitInt,
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `__float128`.
    Float128,
    /// `_Float16` (`std::float16_t`, an arithmetic type).
    Float16,
    /// `_Float32` (`std::float32_t`, P1467).
    Float32,
    /// `_Float64` (`std::float64_t`, P1467).
    Float64,
    /// `_Float128` (`std::float128_t`, P1467).
    StdFloat128,
    /// GCC's `_Float32x` (binary64; WP-380).
    Float32x,
    /// GCC's `_Float64x` (`long double`'s format; WP-380).
    Float64x,
    /// `__bf16` (`std::bfloat16_t`, P1467).
    BFloat16,
    /// `__fp16` (ARM half: arithmetic promotes it to `float`).
    Fp16,
    /// `auto` (a placeholder deduced from the initializer, [dcl.spec.auto]).
    Auto,
    /// `decltype(auto)` ([dcl.type.auto.deduct]/5).
    DecltypeAuto,
    /// GNU `__auto_type` (a placeholder deduced as by `auto`, but never a
    /// deduced return type or an invented template parameter).
    GnuAutoType,
    /// A type-name, class/enum specifier or elaborated-type-specifier:
    /// the type is in [`DeclSpec::named`].
    Named,
}

impl TypeSpecKind {
    /// `auto`, `decltype(auto)` or GNU `__auto_type`.
    pub fn is_placeholder(self) -> bool {
        matches!(self, TypeSpecKind::Auto | TypeSpecKind::DecltypeAuto | TypeSpecKind::GnuAutoType)
    }

    /// The keyword (for "cannot combine" diagnostics).
    pub fn spelling(self) -> &'static str {
        match self {
            TypeSpecKind::None => "type name",
            // clang `DeclSpec::getSpecifierName(TST_typename)`.
            TypeSpecKind::Named => "type-name",
            TypeSpecKind::Void => "void",
            TypeSpecKind::Bool => "bool",
            TypeSpecKind::Char => "char",
            TypeSpecKind::WChar => "wchar_t",
            TypeSpecKind::Char8 => "char8_t",
            TypeSpecKind::Char16 => "char16_t",
            TypeSpecKind::Char32 => "char32_t",
            TypeSpecKind::Int => "int",
            TypeSpecKind::Int128 => "__int128",
            TypeSpecKind::BitInt => "_BitInt",
            TypeSpecKind::Float => "float",
            TypeSpecKind::Double => "double",
            TypeSpecKind::Float128 => "__float128",
            TypeSpecKind::Float16 => "_Float16",
            TypeSpecKind::Float32 => "_Float32",
            TypeSpecKind::Float64 => "_Float64",
            TypeSpecKind::StdFloat128 => "_Float128",
            TypeSpecKind::Float32x => "_Float32x",
            TypeSpecKind::Float64x => "_Float64x",
            TypeSpecKind::BFloat16 => "__bf16",
            TypeSpecKind::Fp16 => "__fp16",
            TypeSpecKind::Auto => "auto",
            TypeSpecKind::DecltypeAuto => "decltype(auto)",
            TypeSpecKind::GnuAutoType => "__auto_type",
        }
    }
}

/// A problem with a specifier sequence, for the parser to report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments.
    pub args: Vec<String>,
}

impl SpecError {
    fn new(id: DiagId, args: &[&str]) -> Self {
        SpecError { id, args: args.iter().map(|s| (*s).to_owned()).collect() }
    }
}

/// The decl-specifier-seq of one declaration.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts of one decl-specifier-seq")]
pub struct DeclSpec {
    /// Storage class (or `typedef`) and where it was written.
    pub storage: Option<(StorageSpec, Loc)>,
    /// `constexpr` written at.
    pub constexpr: Option<Loc>,
    /// `constinit` written at.
    pub constinit: Option<Loc>,
    /// `inline` written at.
    pub inline: Option<Loc>,
    /// `thread_local` (or `__thread` / `_Thread_local`) written at.
    pub thread_local: Option<Loc>,
    /// The thread storage kind `thread_local` stands for:
    /// [`TlsKind::Static`](kiln_ast::decl::TlsKind::Static) when spelled
    /// `__thread` or `_Thread_local` (clang `TSCS___thread` /
    /// `TSCS__Thread_local`: a constant initializer and a trivially
    /// destructible type), else `Dynamic` (unused while `thread_local`
    /// is `None`).
    pub thread_kind: kiln_ast::decl::TlsKind,
    /// `consteval` written at.
    pub consteval: Option<Loc>,
    /// `virtual` written at ([dcl.fct.spec]).
    pub virtual_: Option<Loc>,
    /// `explicit` or `explicit ( constant-expression )` written at
    /// ([dcl.fct.spec]/3); the operand is in `explicit_expr`.
    pub explicit: Option<Loc>,
    /// The operand of `explicit(bool)`, parsed in place.
    pub explicit_expr: Option<ExprId>,
    /// `friend` written at ([class.friend]).
    pub friend: Option<Loc>,
    /// C11 `_Noreturn` written at: a clang extension in C++, the
    /// `[[noreturn]]` of a function ([dcl.attr.noreturn]).
    pub noreturn: Option<Loc>,
    /// cv-qualifiers.
    pub cv: Quals,
    /// Where `const`, `volatile` and `restrict` were first written (in
    /// that order; [`DeclSpec::add_cv`]).
    pub cv_locs: [Option<Loc>; 3],
    /// The C11 `_Atomic` type qualifier (no parenthesis; a clang
    /// extension in C++) written at.
    pub atomic_qual: Option<Loc>,
    /// The clang nullability qualifiers (`_Nonnull`, ...) written among
    /// the decl-specifiers, in order (spec/sema.md §4.1.1).
    pub nullability: Vec<NullSpec>,
    /// The GNU `_Complex` specifier written at.
    pub complex: Option<Loc>,
    /// `short`/`long`.
    pub width: Width,
    /// `signed`/`unsigned`.
    pub sign: Sign,
    /// The base type.
    pub base: TypeSpecKind,
    /// Where the first type-specifier was written.
    pub type_loc: Option<Loc>,
    /// The type of a [`TypeSpecKind::Named`] specifier.
    pub named: Option<QualType>,
    /// The name of a dependent elaborated-type-specifier (`Member` in
    /// `class A<T>::Member`): where a [temp.friend]/5 friend is reported.
    pub dependent_tag_name: Option<Loc>,
    /// The width operand of a [`TypeSpecKind::BitInt`] specifier and the
    /// `_BitInt` keyword's location.
    pub bit_int_width: Option<(ExprId, Loc)>,
    /// The class or enumeration declared or defined by this
    /// decl-specifier-seq (`struct S { }`, `enum E : int;`).
    pub tag: Option<DeclId>,
    /// Whether `tag` is a declaration of its own (`struct S;`,
    /// `struct S { }`) rather than a reference (`struct S *p;`).
    pub tag_declares: bool,
    /// Where this decl-specifier-seq appears, for the checks on a class
    /// or enumeration it defines ([`TagSite`]).
    pub tag_site: TagSite,
    /// `tag` is a definition that no "cannot be defined in" error
    /// invalidated when it was declared (clang's `hasTagDefinition` on a
    /// valid tag): a function declarator then defines it in its result
    /// type ([dcl.fct]/17).
    pub tag_defined: bool,
    /// A "cannot be defined in" error was reported for `tag`: a type-id
    /// of this decl-specifier-seq is the error type (clang marks the
    /// declarator invalid).
    pub tag_rejected: bool,
    /// The keyword of an elaborated or class specifier.
    pub keyword: TagKeyword,
    /// The end of `enum class` / `enum struct` (the scoped keyword's
    /// end): clang 22's insertion point for misplaced attributes.
    pub scoped_key_end: Option<Loc>,
    /// Attributes appertaining to the declared entities.
    pub attrs: Vec<AttrId>,
    /// Where a leading C++11 attribute-specifier-seq (`[[...]]`,
    /// `alignas`) starts, when one was written ([dcl.pre]/5).
    pub lead_cxx11: Option<Loc>,
    /// Where the declaration starts.
    pub loc: Loc,
    /// An error was already reported.
    pub invalid: bool,
    /// The type specifier named no type and was reported ("unknown type
    /// name", "no type named"): the type recovered to `int` (clang's
    /// `TST_error`), and declarators after the first are parsed but not
    /// declared (clang's `ParseDeclGroup`, WP-318).
    pub type_error: bool,
    /// The type-constraint `C<args>` of a placeholder `C auto`
    /// ([dcl.spec.auto.general]/1).
    pub auto_constraint: Option<(DeclId, Vec<kiln_ast::types::TemplateArg>)>,
    /// Where that type-constraint's concept name starts.
    pub constraint_loc: Loc,
}

impl DeclSpec {
    /// An empty specifier sequence starting at `loc`.
    pub fn new(loc: Loc) -> Self {
        DeclSpec { loc, ..DeclSpec::default() }
    }

    /// True if any type-specifier was written.
    pub fn has_type_specifier(&self) -> bool {
        self.base != TypeSpecKind::None
            || self.width != Width::None
            || self.sign != Sign::None
            || self.complex.is_some()
    }

    /// True if anything at all was written.
    pub fn is_empty(&self) -> bool {
        !self.has_type_specifier()
            && self.storage.is_none()
            && self.constexpr.is_none()
            && self.constinit.is_none()
            && self.inline.is_none()
            && self.thread_local.is_none()
            && self.cv.is_empty()
            && self.atomic_qual.is_none()
            && self.nullability.is_empty()
            && self.consteval.is_none()
            && self.virtual_.is_none()
            && self.explicit.is_none()
            && self.friend.is_none()
            && self.noreturn.is_none()
    }

    /// True if this is a `typedef` declaration.
    pub fn is_typedef(&self) -> bool {
        matches!(self.storage, Some((StorageSpec::Typedef, _)))
    }

    /// Records a storage-class specifier.
    ///
    /// # Errors
    /// A duplicate or conflicting storage class.
    pub fn set_storage(&mut self, s: StorageSpec, loc: Loc) -> Result<(), SpecError> {
        match self.storage {
            Some((old, _)) if old == s => {
                Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &[s.spelling()]))
            }
            Some((old, _)) => {
                Err(SpecError::new(DiagId::parse_invalid_decl_spec_combination, &[old.spelling()]))
            }
            None => {
                self.storage = Some((s, loc));
                Ok(())
            }
        }
    }

    /// Records a cv-qualifier written at `loc` (duplicates are an
    /// extension warning).
    ///
    /// # Errors
    /// A duplicate qualifier.
    pub fn add_cv(&mut self, q: Quals, spelling: &str, loc: Loc) -> Result<(), SpecError> {
        let dup = self.cv.contains(q);
        self.cv |= q;
        let slot = [Quals::CONST, Quals::VOLATILE, Quals::RESTRICT].iter().position(|&k| k == q);
        if let Some(i) = slot {
            self.cv_locs[i].get_or_insert(loc);
        }
        if dup {
            Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &[spelling]))
        } else {
            Ok(())
        }
    }

    /// Records the GNU `_Complex` specifier.
    ///
    /// # Errors
    /// `_Complex _Complex`.
    pub fn set_complex(&mut self, loc: Loc) -> Result<(), SpecError> {
        self.note_type(loc);
        if self.complex.replace(loc).is_some() {
            return Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &["_Complex"]));
        }
        Ok(())
    }

    fn note_type(&mut self, loc: Loc) {
        if self.type_loc.is_none() {
            self.type_loc = Some(loc);
        }
    }

    /// Records `short`/`long`.
    ///
    /// # Errors
    /// `long long long`, `short long`, ...
    pub fn set_width(&mut self, w: Width, loc: Loc) -> Result<(), SpecError> {
        self.note_type(loc);
        if self.type_error && self.base != TypeSpecKind::None {
            return Ok(()); // after an unknown type name, as `set_base`
        }
        let new = match (self.width, w) {
            (Width::None, w) => w,
            (Width::Long, Width::Long) => Width::LongLong,
            (Width::LongLong, Width::Long) => {
                return Err(SpecError::new(DiagId::parse_long_long_long, &[]));
            }
            (Width::Short, Width::Short) => {
                return Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &["short"]));
            }
            (old, _w) => {
                let sp = |w: Width| match w {
                    Width::Short => "short",
                    Width::LongLong => "long long",
                    _ => "long",
                };
                return Err(SpecError::new(
                    DiagId::parse_invalid_decl_spec_combination,
                    &[sp(old)],
                ));
            }
        };
        self.width = new;
        self.check_base_width(false)
    }

    /// Records `signed`/`unsigned`.
    ///
    /// # Errors
    /// A duplicate or conflicting sign.
    pub fn set_sign(&mut self, s: Sign, loc: Loc) -> Result<(), SpecError> {
        self.note_type(loc);
        if self.type_error && self.base != TypeSpecKind::None {
            return Ok(()); // after an unknown type name, as `set_base`
        }
        let sp = |s: Sign| if s == Sign::Signed { "signed" } else { "unsigned" };
        match self.sign {
            Sign::None => {}
            old if old == s => {
                return Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &[sp(s)]));
            }
            old => {
                return Err(SpecError::new(
                    DiagId::parse_invalid_decl_spec_combination,
                    &[sp(old)],
                ));
            }
        }
        self.sign = s;
        self.check_base_sign(false)
    }

    /// Records the base type.
    ///
    /// # Errors
    /// Two base types.
    pub fn set_base(
        &mut self,
        b: TypeSpecKind,
        named: Option<QualType>,
        loc: Loc,
    ) -> Result<(), SpecError> {
        self.note_type(loc);
        if self.type_error && self.base != TypeSpecKind::None {
            // clang's `SetTypeSpecType` after `TST_error`: a later type
            // specifier is dropped silently (`restrict int f();`).
            return Ok(());
        }
        if self.base != TypeSpecKind::None {
            return Err(SpecError::new(
                DiagId::parse_invalid_decl_spec_combination,
                &[self.base.spelling()],
            ));
        }
        self.base = b;
        self.named = named;
        self.check_base_width(true)?;
        self.check_base_sign(true)
    }

    fn check_base_width(&self, new_is_base: bool) -> Result<(), SpecError> {
        let ok = matches!(
            (self.width, self.base),
            (Width::None, _)
                | (_, TypeSpecKind::None)
                | (Width::Short | Width::LongLong | Width::Long, TypeSpecKind::Int)
                | (Width::Long, TypeSpecKind::Double)
        );
        if ok {
            return Ok(());
        }
        let w = match self.width {
            Width::Short => "short",
            Width::LongLong => "long long",
            _ => "long",
        };
        let prev = if new_is_base { w } else { self.base.spelling() };
        Err(SpecError::new(DiagId::parse_invalid_decl_spec_combination, &[prev]))
    }

    fn check_base_sign(&self, new_is_base: bool) -> Result<(), SpecError> {
        let ok = matches!(
            self.base,
            TypeSpecKind::None
                | TypeSpecKind::Char
                | TypeSpecKind::Int
                | TypeSpecKind::Int128
                | TypeSpecKind::BitInt
        ) || self.sign == Sign::None;
        if ok {
            return Ok(());
        }
        let s = if self.sign == Sign::Signed { "signed" } else { "unsigned" };
        let prev = if new_is_base { s } else { self.base.spelling() };
        Err(SpecError::new(DiagId::parse_invalid_decl_spec_combination, &[prev]))
    }
}

/// Where a decl-specifier-seq appears, for the rules on the class or
/// enumeration definitions it may contain ([dcl.type.general]/3,
/// [dcl.fct]/17, [stmt.pre]/5, [temp.alias]/2; clang's `ActOnTag` and
/// `GetDeclSpecTypeForDeclarator` "cannot be defined in" checks; spec/sema.md
/// §4.4.1). The parser sets it for the next decl-specifier-seq only.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TagSite {
    /// As the [`DeclaratorContext`] says: allowed at namespace, class and
    /// block scope and in a for-init-statement; a parameter type, a
    /// condition; a type-id of [`DeclaratorContext::TypeName`] is a
    /// [`TagSite::TypeSpecifier`].
    #[default]
    Context,
    /// A type-specifier-seq rejected when the tag is declared (clang's
    /// `DSC_type_specifier` and template parameters): `typeid`, named
    /// casts, a conversion-type-id, a non-type template parameter, a
    /// template parameter default, a dynamic exception specification.
    TypeSpecifier,
    /// A type-id whose declarator rejects it ("type specifier" with the
    /// tag's type): `sizeof`, `alignof`, a C-style cast, a parenthesized
    /// new type-id, an exception-declaration.
    TypeId,
    /// The grammar admits no definition: a new-type-id, a trailing return
    /// type, a type-id template argument (the tag is a reference and the
    /// `{` is left to the caller).
    NoDefinition,
    /// The type-id of an alias-declaration (allowed).
    Alias,
    /// The type-id of an alias template.
    AliasTemplate,
}

/// Where a declarator appears ([dcl.decl], clang's `DeclaratorContext`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeclaratorContext {
    /// Namespace scope.
    File,
    /// A member-declaration.
    Member,
    /// Block scope.
    Block,
    /// A parameter-declaration.
    Prototype,
    /// A type-id (casts, `sizeof(T)`, alias-declarations).
    TypeName,
    /// The condition of `if`/`while`/`switch`/`for`.
    Condition,
    /// The init-statement of a `for`.
    ForInit,
}

/// A parameter of a function declarator.
#[derive(Clone, Debug)]
pub struct ParamInfo {
    /// The name, when written.
    pub name: Option<(Symbol, Loc)>,
    /// The type after [dcl.fct]/5 adjustment (arrays and functions
    /// become pointers).
    pub ty: QualType,
    /// The type as declared, when [dcl.fct]/5 adjusted it: what a template
    /// substitution substitutes into ([temp.deduct.general]/11.1: `T[3]`
    /// with `T = void` fails although `void*` would not).
    pub original_ty: Option<QualType>,
    /// Where the parameter-declaration starts (unnamed parameters).
    pub loc: Loc,
    /// Attributes.
    pub attrs: Vec<AttrId>,
    /// An error was reported in the parameter.
    pub invalid: bool,
    /// The default argument, set by the parser after
    /// [`crate::Sema::act_on_param_declarator`] returns (a default parsed
    /// in place is already converted to the parameter's type by
    /// [`crate::Sema::act_on_param_default_init`]).
    pub default_arg: DefaultArg,
    /// The `=` of a default argument written in place (invalid when
    /// none is, or for a delayed one: [`DefaultArg::Unparsed`] holds it).
    pub default_loc: Loc,
    /// Where the parameter-declaration starts and ends (just past its
    /// last token, default argument included; the `-Wvexing-parse`
    /// fix-its, [dcl.ambig.res]).
    pub range: (Loc, Loc),
    /// Where the first type-specifier was written (`T` of `const T x`).
    pub type_loc: Loc,
    /// The outermost `&`/`&&` of the declarator when the declared type
    /// is a reference (invalid otherwise; [`crate::Sema::ref_locs`]).
    pub ref_loc: Loc,
    /// The innermost `*`/`[` of the declarator when it applies to the
    /// decl-specifiers (invalid otherwise; [`crate::Sema::inner_chunk_locs`]).
    pub chunk_loc: Loc,
    /// The parameter declaration created at its point of declaration
    /// ([basic.scope.pdecl]/1) by [`crate::Sema::act_on_prototype_param`],
    /// so that it is in scope in the rest of the declarator
    /// ([basic.scope.param]); the function adopts it.
    pub decl: Option<DeclId>,
    /// `this` before the parameter-declaration: an explicit object
    /// parameter ([dcl.fct]/6, P0847).
    pub explicit_object: Option<Loc>,
}

/// The default argument of a [`ParamInfo`] ([dcl.fct.default]).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DefaultArg {
    /// None written.
    #[default]
    None,
    /// `= e`, parsed in place and converted to the parameter's type
    /// ([`crate::Sema::act_on_param_default_init`]).
    Parsed(ExprId),
    /// `=` at the first location, the argument starting at the second,
    /// in a member function declared in a class being defined: the
    /// tokens are saved and replayed in the complete-class context
    /// (spec/sema-class.md §2.7); the parameter gets
    /// `ParamData::DEFAULT_ARG_PENDING`.
    Unparsed(Loc, Loc),
}

/// A clang nullability qualifier as written (`_Nonnull`, `_Nullable`,
/// `_Null_unspecified`, `_Nullable_result`; spec/sema.md §4.1.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NullSpec {
    /// Which one (a nullability [`TypeAttrKind`]).
    pub kind: TypeAttrKind,
    /// The keyword.
    pub loc: Loc,
}

/// One declarator operator ([dcl.meaning]), clang's `DeclaratorChunk`.
#[derive(Clone, Debug)]
pub enum Chunk {
    /// `* cv`.
    Pointer {
        /// The cv-qualifiers after the `*`.
        quals: Quals,
        /// The `*`.
        loc: Loc,
    },
    /// `nested-name-specifier * cv` ([dcl.mptr]): a pointer to a member
    /// of the class the qualifier names (checked by
    /// [`Sema::type_for_declarator`](crate::Sema::type_for_declarator)).
    MemberPointer {
        /// The nested-name-specifier as the parser resolved it; `None`
        /// after an error in it was reported (the declarator's type is
        /// then the error type).
        qual: Option<crate::using::Qualifier>,
        /// The cv-qualifiers after the `*`.
        quals: Quals,
        /// Where the nested-name-specifier starts.
        loc: Loc,
    },
    /// `&`.
    LRef {
        /// The `&`.
        loc: Loc,
    },
    /// `&&`.
    RRef {
        /// The `&&`.
        loc: Loc,
    },
    /// `[ size ]`.
    Array {
        /// The bound, when written.
        size: Option<ExprId>,
        /// The `[`.
        loc: Loc,
    },
    /// `( params )`.
    Function {
        /// The parameters (`(void)` is none).
        params: Vec<ParamInfo>,
        /// `...` written.
        variadic: bool,
        /// The `(`.
        loc: Loc,
        /// The `)`.
        rparen: Loc,
        /// The exception specification ([except.spec]) as written, from
        /// [`Sema::act_on_noexcept_spec`](crate::Sema::act_on_noexcept_spec)
        /// and friends; `ExcSpec::None` when absent.
        exc: ExcSpec,
        /// cv-qualifier-seq after the parameter list ([dcl.fct]/1).
        quals: Quals,
        /// ref-qualifier after the parameter list.
        ref_qual: RefQual,
        /// The trailing return type ([dcl.fct]/2) and its `->`.
        trailing: Option<(QualType, Loc)>,
        /// The parentheses could also have been a direct initializer:
        /// the parameter clause was ambiguous and resolved as a
        /// declaration ([dcl.ambig.res]/1, clang's
        /// `hasAmbiguousFunctionDeclarator`; `-Wvexing-parse`).
        ambiguous: bool,
    },
}

/// A declarator: a name plus the operators applied to it.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent parser facts")]
pub struct Declarator {
    /// Where it appears.
    pub ctx: DeclaratorContext,
    /// The declarator-id, when there is one.
    pub name: Option<(Symbol, Loc)>,
    /// The context named by a nested-name-specifier on the declarator-id
    /// (`int N::x = 1;`).
    pub qualifier: Option<DeclId>,
    /// That nested-name-specifier as written (`A<T>::` of a
    /// [temp.friend]/5 friend `template<class T> friend void A<T>::f();`).
    pub qualifier_nns: Option<NnsId>,
    /// That nested-name-specifier begins with the global `::`
    /// (`A (::N::x);`): clang's walk of its prefixes for a `Global`
    /// specifier.
    pub global_qualifier: bool,
    /// The operators, innermost (closest to the name) first; the type is
    /// built from the last.
    pub chunks: Vec<Chunk>,
    /// The width of a bit-field member.
    pub bit_width: Option<ExprId>,
    /// Attributes appertaining to the declarator.
    pub attrs: Vec<AttrId>,
    /// Where the declarator starts.
    pub loc: Loc,
    /// The start of its last token, counting a trailing requires-clause,
    /// an asm label, GNU attributes and the last virt-specifier but not
    /// trailing C++11 attributes (clang's `getEndLoc`); invalid unless
    /// the parser set it (member declarators only, for now).
    pub end: Loc,
    /// An error was already reported.
    pub invalid: bool,
    /// A parameter needed the missing-`typename` recovery
    /// ([temp.res.general]/4): the function is declared, but a later
    /// qualified (out-of-line) declaration does not match it (clang marks
    /// it invalid). Set by the parser.
    pub missing_typename: bool,
    /// The initializer syntax that follows (the parser peeks at `=`, `(`
    /// or `{` before the declaration is created; [dcl.init]/1).
    pub init_style: Option<kiln_ast::decl::InitStyle>,
    /// A function declarator followed by `= default` (the parser peeks
    /// before the declaration is created): its consteval-only check
    /// waits for the defaulting (an immediate-escalating function).
    pub defaulted: bool,
    /// The declarator may be followed by a direct initializer `( ... )`
    /// (clang's `mayBeFollowedByCXXDirectInit`, less the grouping-paren
    /// and name checks): a variable declaration that is not a `typedef`
    /// or a block-scope `extern`. Set by the parser.
    pub direct_init: bool,
    /// Grouping parentheses were parsed (`int (x)`).
    pub grouping: bool,
    /// The GNU asm label (`asm("name")` after the declarator, [dcl.asm]
    /// extension): an ordinary, non-empty `StringLiteral` expression and
    /// the extent of its tokens.
    pub asm_label: Option<(ExprId, kiln_source::Span)>,
    /// A constructor, destructor, conversion-function or operator-function
    /// declarator-id and where it was written (`name` is `None` then).
    pub special: Option<(SpecialName, Loc)>,
    /// Where a conversion-function-id's type-specifier was written
    /// (`operator const auto *`: the `auto`), else invalid.
    pub conv_type_loc: Loc,
    /// The virt-specifier `override` ([class.virtual]/1).
    pub virt_override: Option<Loc>,
    /// The virt-specifier `final`.
    pub virt_final: Option<Loc>,
    /// The pure-specifier `= 0` ([class.abstract]/2): where the `0` is.
    pub pure: Option<Loc>,
    /// The function's own `noexcept ( constant-expression )`, saved for
    /// replay in a complete-class context ([class.mem]/7; spec/parse.md
    /// §2.4): where `noexcept` is. The `Chunk::Function`'s `exc` is
    /// `ExcSpec::None` meanwhile; sema makes it `ExcSpec::Deferred(f)`.
    pub late_noexcept: Option<Loc>,
    /// `this` before a parameter-declaration (an explicit object
    /// parameter, P0847 [dcl.fct]/6).
    pub explicit_object: Option<Loc>,
    /// Where the innermost function declarator's `const`, `volatile`,
    /// ref-qualifier and `__restrict` are ("explicit object member
    /// function cannot have '...' qualifier" points at the first and
    /// quotes them in order).
    pub fn_qual_locs: [Option<Loc>; 4],
    /// The first nullability keyword after the innermost function
    /// declarator's parameter list (`void f() _Nonnull`): parsed and
    /// dropped, it is a qualifier only to the "cannot have '' qualifier"
    /// checks (spec/sema.md §4.1.1).
    pub fn_nullability: Option<Loc>,
    /// The nullability qualifiers after a ptr-operator, with the index
    /// in `chunks` of the pointer, member-pointer or reference chunk
    /// they follow, in order (spec/sema.md §4.1.1).
    pub nullability: Vec<(usize, NullSpec)>,
    /// The declarator-id is an operator-function-id (`operator new`):
    /// `name` then holds the `operator` keyword and its location.
    pub op_name: Option<OverloadedOperator>,
    /// The declarator-id is a literal-operator-id (`operator""_x`,
    /// [over.literal]/1): its ud-suffix; `name` then holds the
    /// `operator` keyword and its location.
    pub lit_suffix: Option<Symbol>,
    /// `...` before the declarator-id: a function or template parameter
    /// pack ([dcl.fct]/16, [temp.variadic]/3).
    pub ellipsis: Option<Loc>,
    /// Where an abstract declarator's name would go: the token at its
    /// innermost position (clang `ParseDirectDeclarator`'s
    /// `SetIdentifier(nullptr, Tok.getLocation())`).
    pub abstract_at: Option<Loc>,
    /// The trailing requires-clause of a function declarator
    /// ([dcl.decl.general]/4, [temp.pre]/9).
    pub trailing_requires: Option<ExprId>,
    /// The declarator was diagnosed and declares nothing (a function
    /// template partial specialization, or a specialization form that is
    /// WP-070a's; [`Sema::act_on_specialization_declarator_id`]).
    ///
    /// [`Sema::act_on_specialization_declarator_id`]: crate::Sema::act_on_specialization_declarator_id
    pub dropped: bool,
    /// The declarator-id is a template-id naming a variable template
    /// after a template head or `template<>`: a partial or explicit
    /// specialization ([temp.spec.partial], [temp.expl.spec];
    /// [`Sema::act_on_specialization_declarator_id`]).
    ///
    /// [`Sema::act_on_specialization_declarator_id`]: crate::Sema::act_on_specialization_declarator_id
    pub spec_id: Option<crate::templ::spec::SpecDeclaratorId>,
    /// A structured binding declaration's `[ identifier-list ]`
    /// ([dcl.struct.bind], [dcl.pre]/6): the declarator has no name then
    /// (a ref-qualifier is its only chunk).
    pub decomp: Option<Decomposition>,
    /// The declarator of a friend declaration ([class.friend]): its
    /// declarator-id may be a template-id naming a specialization of a
    /// function template without a template head ([temp.friend]/1.3).
    pub friend: bool,
    /// The declarator belongs to a `typedef` declaration (clang's
    /// `isFunctionDeclarationContext` is false then: its function
    /// declarator declares no function, [dcl.fct]/22). Set by the parser.
    pub typedef: bool,
    /// The decl-specifier-seq has a type specifier, so the declarator-id
    /// names no deduction guide (`auto mk() -> int` where `mk` is a class
    /// template; clang's `ParseDirectDeclarator` clears
    /// `AllowDeductionGuide`, [temp.deduct.guide]/1). Set by the parser.
    pub typed: bool,
    /// The type-constraint of a constrained placeholder in one of the
    /// declarator's trailing return types (`auto (*v)() -> C auto`,
    /// nested ones included), where its "does not satisfy" diagnostics
    /// point (clang's `AutoTypeLoc`); invalid when there is none. Set by
    /// the parser.
    pub trailing_constraint: Loc,
    /// What the declarator declares, beyond `ctx`, where that decides
    /// whether a placeholder for a deduced class type may appear
    /// ([dcl.type.class.deduct]/2; clang's finer `DeclaratorContext`s).
    pub site: DeclSite,
    /// A lambda's call operator declarator, whose return type is an
    /// implied `auto` rather than a written decl-specifier
    /// ([expr.prim.lambda.closure]/3).
    pub lambda: bool,
    /// The outermost grouping parentheses and the number of chunks inside
    /// them (`B (x)`: `(0, loc of '(')`; [dcl.type.class.deduct]/2).
    pub outer_paren: Option<(usize, Loc)>,
    /// Every pair of grouping parentheses, in the order they close: the
    /// number of chunks inside them and the locations of `(` and `)`
    /// (clang's `DeclaratorChunk::Paren`; `ParenType` sugar and
    /// `-Wvexing-parse`'s redundant-parentheses warning, WP-191).
    pub parens: Vec<(usize, Loc, Loc)>,
    /// Not the first declarator of its declaration (clang's
    /// `!isFirstDeclarator()`: a comma precedes it).
    pub after_comma: bool,
}

/// Where a declarator appears, refining [`DeclaratorContext`] for the
/// contexts clang distinguishes when a placeholder for a deduced class
/// type is not allowed (`err_auto_not_allowed`, `err_template_missing_args`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DeclSite {
    /// As `ctx` says (a type-id: a cast, `sizeof`, `typeid`, ...).
    #[default]
    Plain,
    /// A parameter of a lambda-declarator.
    LambdaParam,
    /// A parameter of a requires-expression.
    RequiresParam,
    /// A non-type template parameter (a placeholder is allowed: C++20
    /// [temp.param]/6).
    TemplateParam,
    /// A conversion-type-id.
    ConversionId,
    /// The type-id of a new-expression (a placeholder is allowed).
    New,
    /// A type-id template argument (a class template name alone is a
    /// template template argument, parsed apart).
    TemplateArg,
    /// A trailing return type: clang's parser forms no placeholder for a
    /// deduced class type there (the template name lacks its arguments);
    /// `auto` and `decltype(auto)` are allowed.
    NoDeduction,
    /// The type-id of an alias-declaration: as [`DeclSite::NoDeduction`]
    /// for a deduced class type; `auto` is "not allowed in type alias".
    Alias,
    /// The default argument of a type template parameter: as
    /// [`DeclSite::NoDeduction`] for a deduced class type; `auto` is "not
    /// allowed in template argument" (clang's `TemplateTypeArg`).
    TypeParamDefault,
    /// A declarator rebuilt by template substitution (already checked).
    Subst,
    /// The type-id operand of `^^` ([expr.reflect]): as
    /// [`DeclSite::Plain`], except that a cv- or ref-qualified function
    /// type may be reflected (it declares nothing: [dcl.fct]/6 restricts
    /// declarations; GCC 16.1 accepts `^^int() const`).
    Reflect,
}

/// The bracketed identifier list of a structured binding declaration
/// ([dcl.pre]/6: `attribute-specifier-seq(opt) decl-specifier-seq
/// ref-qualifier(opt) [ sb-identifier-list ] initializer ;`).
#[derive(Clone, Debug)]
pub struct Decomposition {
    /// Where `[` is.
    pub lbracket: Loc,
    /// Where `]` is.
    pub rbracket: Loc,
    /// The sb-identifiers in order.
    pub ids: Vec<BindingId>,
}

/// One sb-identifier ([dcl.pre]/6): `...(opt) identifier
/// attribute-specifier-seq(opt)`.
#[derive(Clone, Debug)]
pub struct BindingId {
    /// The identifier and where it is.
    pub name: Symbol,
    /// Its location.
    pub loc: Loc,
    /// A C++26 binding pack's `...` (P1061).
    pub ellipsis: Option<Loc>,
    /// The attributes after the identifier (P0609).
    pub attrs: Vec<AttrId>,
}

/// A declarator-id that is not an identifier ([dcl.decl]/4,
/// [class.ctor], [class.dtor], [class.conv.fct], [over.oper]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpecialName {
    /// The injected-class-name of the class (`S(...)`, `S::S(...)`).
    Constructor(Symbol),
    /// `~S` (the class name as written).
    Destructor(Symbol),
    /// `operator conversion-type-id`.
    Conversion(QualType),
    /// `operator @`.
    Operator(OverloadedOperator),
    /// `operator "" X` / `operator ""X` ([over.literal]/1): the
    /// ud-suffix `X`.
    LiteralOperator(Symbol),
    /// The template-name of a deduction-guide ([temp.deduct.guide]):
    /// the class template it is for.
    DeductionGuide(DeclId),
}

impl Declarator {
    /// An empty declarator in `ctx` at `loc`.
    pub fn new(ctx: DeclaratorContext, loc: Loc) -> Self {
        Declarator {
            ctx,
            name: None,
            qualifier: None,
            qualifier_nns: None,
            chunks: Vec::new(),
            bit_width: None,
            attrs: Vec::new(),
            loc,
            end: Loc::INVALID,
            invalid: false,
            missing_typename: false,
            init_style: None,
            defaulted: false,
            direct_init: false,
            grouping: false,
            asm_label: None,
            special: None,
            conv_type_loc: Loc::INVALID,
            site: DeclSite::Plain,
            lambda: false,
            outer_paren: None,
            parens: Vec::new(),
            after_comma: false,
            global_qualifier: false,
            virt_override: None,
            virt_final: None,
            pure: None,
            late_noexcept: None,
            explicit_object: None,
            fn_qual_locs: [None; 4],
            fn_nullability: None,
            nullability: Vec::new(),
            op_name: None,
            lit_suffix: None,
            ellipsis: None,
            abstract_at: None,
            trailing_requires: None,
            dropped: false,
            spec_id: None,
            decomp: None,
            friend: false,
            typedef: false,
            typed: false,
            trailing_constraint: Loc::INVALID,
        }
    }

    /// Pushes the ptr-operator `chunk` with the nullability qualifiers
    /// written after it.
    pub fn push_chunk(&mut self, chunk: Chunk, nulls: Vec<NullSpec>) {
        let i = self.chunks.len();
        self.nullability.extend(nulls.into_iter().map(|n| (i, n)));
        self.chunks.push(chunk);
    }

    /// True if the declarator has a declarator-id (an identifier or a
    /// [`SpecialName`]).
    pub fn has_name(&self) -> bool {
        self.name.is_some() || self.special.is_some() || self.decomp.is_some()
    }

    /// The declaration name of a function declarator whose identifier
    /// (or `operator` keyword) is `name`: an operator-function-id, a
    /// literal-operator-id, or `name`.
    pub fn fn_decl_name(&self, name: Symbol) -> DeclName {
        match (self.op_name, self.lit_suffix) {
            (Some(op), _) => DeclName::operator(op),
            (None, Some(s)) => DeclName::literal_operator(s),
            (None, None) => DeclName::ident(name),
        }
    }

    /// The location of the name, or of the declarator.
    pub fn name_loc(&self) -> Loc {
        self.name
            .map(|n| n.1)
            .or(self.special.map(|s| s.1))
            .or(self.decomp.as_ref().map(|b| b.lbracket))
            .unwrap_or(self.loc)
    }

    /// True if the outermost operator applied to the name is a function
    /// declarator (`f(int)`, `(*g)(int)` is not).
    pub fn is_function_declarator(&self) -> bool {
        matches!(self.chunks.first(), Some(Chunk::Function { .. }))
    }

    /// True if the outermost operator applied to the name is an array
    /// declarator of unknown bound (`d[]`; clang `isArrayOfUnknownBound`).
    pub fn is_array_of_unknown_bound(&self) -> bool {
        matches!(self.chunks.first(), Some(Chunk::Array { size: None, .. }))
    }

    /// The parameters of a function declarator.
    pub fn function_params(&self) -> Option<&[ParamInfo]> {
        match self.chunks.first() {
            Some(Chunk::Function { params, .. }) => Some(params),
            _ => None,
        }
    }
}
