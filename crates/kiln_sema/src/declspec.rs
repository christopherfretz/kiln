//! Declaration specifiers and declarators as the parser collects them
//! (clang's `DeclSpec`/`Declarator`, spec/sema.md §4).
//!
//! The parser records what was written; [`crate::Sema`] turns a
//! [`DeclSpec`] into a type ([dcl.type.simple]) and a [`Declarator`] into
//! a declaration.

use kiln_ast::types::{Quals, TagKeyword};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
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
pub enum BaseSpec {
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
    /// `float`.
    Float,
    /// `double`.
    Double,
    /// `__float128`.
    Float128,
    /// `auto` (a placeholder deduced from the initializer, [dcl.spec.auto]).
    Auto,
    /// A type-name, class/enum specifier or elaborated-type-specifier:
    /// the type is in [`DeclSpec::named`].
    Named,
}

impl BaseSpec {
    /// The keyword (for "cannot combine" diagnostics).
    pub fn spelling(self) -> &'static str {
        match self {
            BaseSpec::None | BaseSpec::Named => "type name",
            BaseSpec::Void => "void",
            BaseSpec::Bool => "bool",
            BaseSpec::Char => "char",
            BaseSpec::WChar => "wchar_t",
            BaseSpec::Char8 => "char8_t",
            BaseSpec::Char16 => "char16_t",
            BaseSpec::Char32 => "char32_t",
            BaseSpec::Int => "int",
            BaseSpec::Int128 => "__int128",
            BaseSpec::Float => "float",
            BaseSpec::Double => "double",
            BaseSpec::Float128 => "__float128",
            BaseSpec::Auto => "auto",
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
pub struct DeclSpec {
    /// Storage class (or `typedef`) and where it was written.
    pub storage: Option<(StorageSpec, Loc)>,
    /// `constexpr` written at.
    pub constexpr: Option<Loc>,
    /// `constinit` written at.
    pub constinit: Option<Loc>,
    /// `inline` written at.
    pub inline: Option<Loc>,
    /// `thread_local` written at.
    pub thread_local: Option<Loc>,
    /// cv-qualifiers.
    pub cv: Quals,
    /// `short`/`long`.
    pub width: Width,
    /// `signed`/`unsigned`.
    pub sign: Sign,
    /// The base type.
    pub base: BaseSpec,
    /// Where the first type-specifier was written.
    pub type_loc: Option<Loc>,
    /// The type of a [`BaseSpec::Named`] specifier.
    pub named: Option<QualType>,
    /// The class or enumeration declared or defined by this
    /// decl-specifier-seq (`struct S { }`, `enum E : int;`).
    pub tag: Option<DeclId>,
    /// Whether `tag` is a declaration of its own (`struct S;`,
    /// `struct S { }`) rather than a reference (`struct S *p;`).
    pub tag_declares: bool,
    /// The keyword of an elaborated or class specifier.
    pub keyword: TagKeyword,
    /// Attributes appertaining to the declared entities.
    pub attrs: Vec<AttrId>,
    /// Where the declaration starts.
    pub loc: Loc,
    /// An error was already reported.
    pub invalid: bool,
}

impl DeclSpec {
    /// An empty specifier sequence starting at `loc`.
    pub fn new(loc: Loc) -> Self {
        DeclSpec { loc, ..DeclSpec::default() }
    }

    /// True if any type-specifier was written.
    pub fn has_type_specifier(&self) -> bool {
        self.base != BaseSpec::None || self.width != Width::None || self.sign != Sign::None
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

    /// Records a cv-qualifier (duplicates are an extension warning).
    ///
    /// # Errors
    /// A duplicate qualifier.
    pub fn add_cv(&mut self, q: Quals, spelling: &str) -> Result<(), SpecError> {
        let dup = self.cv.contains(q);
        self.cv |= q;
        if dup {
            Err(SpecError::new(DiagId::parse_duplicate_decl_spec, &[spelling]))
        } else {
            Ok(())
        }
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
        b: BaseSpec,
        named: Option<QualType>,
        loc: Loc,
    ) -> Result<(), SpecError> {
        self.note_type(loc);
        if self.base != BaseSpec::None {
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
                | (_, BaseSpec::None)
                | (Width::Short | Width::LongLong | Width::Long, BaseSpec::Int)
                | (Width::Long, BaseSpec::Double)
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
        let ok =
            matches!(self.base, BaseSpec::None | BaseSpec::Char | BaseSpec::Int | BaseSpec::Int128)
                || self.sign == Sign::None;
        if ok {
            return Ok(());
        }
        let s = if self.sign == Sign::Signed { "signed" } else { "unsigned" };
        let prev = if new_is_base { s } else { self.base.spelling() };
        Err(SpecError::new(DiagId::parse_invalid_decl_spec_combination, &[prev]))
    }
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
    /// The type as declared (before [dcl.fct]/5 adjustment).
    pub ty: QualType,
    /// Where the parameter-declaration starts (unnamed parameters).
    pub loc: Loc,
    /// Attributes.
    pub attrs: Vec<AttrId>,
    /// An error was reported in the parameter.
    pub invalid: bool,
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
    },
}

/// A declarator: a name plus the operators applied to it.
#[derive(Clone, Debug)]
pub struct Declarator {
    /// Where it appears.
    pub ctx: DeclaratorContext,
    /// The declarator-id, when there is one.
    pub name: Option<(Symbol, Loc)>,
    /// The context named by a nested-name-specifier on the declarator-id
    /// (`int N::x = 1;`).
    pub qualifier: Option<DeclId>,
    /// The operators, innermost (closest to the name) first; the type is
    /// built from the last.
    pub chunks: Vec<Chunk>,
    /// The width of a bit-field member.
    pub bit_width: Option<ExprId>,
    /// Attributes appertaining to the declarator.
    pub attrs: Vec<AttrId>,
    /// Where the declarator starts.
    pub loc: Loc,
    /// An error was already reported.
    pub invalid: bool,
    /// The initializer syntax that follows (the parser peeks at `=`, `(`
    /// or `{` before the declaration is created; [dcl.init]/1).
    pub init_style: Option<kiln_ast::decl::InitStyle>,
}

impl Declarator {
    /// An empty declarator in `ctx` at `loc`.
    pub fn new(ctx: DeclaratorContext, loc: Loc) -> Self {
        Declarator {
            ctx,
            name: None,
            qualifier: None,
            chunks: Vec::new(),
            bit_width: None,
            attrs: Vec::new(),
            loc,
            invalid: false,
            init_style: None,
        }
    }

    /// The location of the name, or of the declarator.
    pub fn name_loc(&self) -> Loc {
        self.name.map_or(self.loc, |n| n.1)
    }

    /// True if the outermost operator applied to the name is a function
    /// declarator (`f(int)`, `(*g)(int)` is not).
    pub fn is_function_declarator(&self) -> bool {
        matches!(self.chunks.first(), Some(Chunk::Function { .. }))
    }

    /// The parameters of a function declarator.
    pub fn function_params(&self) -> Option<&[ParamInfo]> {
        match self.chunks.first() {
            Some(Chunk::Function { params, .. }) => Some(params),
            _ => None,
        }
    }
}
