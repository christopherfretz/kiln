//! Itanium C++ name mangling ([abi:mangling]): functions, variables and
//! static data members in namespaces and classes, member functions with
//! cv- and ref-qualifiers, constructors and destructors (`C1`/`C2`,
//! `D0`/`D1`/`D2`), conversion functions, class, function and variable
//! template specializations (type, integral, enumeration, `nullptr`,
//! declaration and template template arguments, packs), local entities
//! (`Z...E`), member pointers, guard variables (`_ZGV`) and reference
//! temporaries (`_ZGR`), with substitutions as clang 19 makes them. See
//! spec/abi.md §"Mangling".

use kiln_ast::Ast;
use kiln_ast::consts::{ByteImage, ConstValue, Member, PrintEnv, PtrBase, Scalar, Shape};
use kiln_ast::decl::{DeclKind, FnStorage, LinkageLang, NamespaceData, TemplLinkKind, VarStorage};
use kiln_ast::ids::{ArgListId, ConstValueId, DeclId, QualType, TemplateNameId};
use kiln_ast::names::{DeclNameView, OverloadedOperator};
use kiln_ast::types::{ArgView, BuiltinType, RefQual, TemplateName, TypeView};

/// A substitution candidate ([abi:mangling.compression]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sub {
    /// A namespace, class or enumeration (also as a type).
    Decl(DeclId),
    /// A template (as a template prefix or template template argument).
    Template(DeclId),
    /// A (canonical, possibly qualified) type.
    Type(QualType),
}

/// The constructor or destructor variant of a symbol ([abi:mangling]
/// <ctor-dtor-name>).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Structor {
    /// `C1` / `D1`: the complete-object constructor or destructor.
    Complete,
    /// `C2` / `D2`: the base-object constructor or destructor.
    Base,
    /// `D0`: the deleting destructor.
    Deleting,
}

/// True if `d` is declared inside an `extern "C"` linkage specification
/// (the innermost one wins).
pub fn is_extern_c(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        let n = ast.decls.get(ctx);
        match n.kind() {
            DeclKind::LinkageSpec => {
                return ast.decls.scope(ctx).is_some_and(|s| s.aux == LinkageLang::C as u32);
            }
            DeclKind::TranslationUnit => return false,
            // Class members and local entities never have C language
            // linkage.
            k if is_record_kind(k) || is_function_kind(k) => return false,
            _ => {}
        }
        c = n.ctx();
    }
    false
}

fn is_record_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

fn is_function_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::Function
            | DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion
    )
}

/// True if `ns` is an unnamed namespace.
fn is_anon_ns(ast: &Ast, ns: DeclId) -> bool {
    ast.decls.namespace(ns).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0)
}

/// True if some enclosing namespace of `d` is unnamed.
fn in_anon_ns(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        if ast.decls.get(ctx).kind() == DeclKind::Namespace && is_anon_ns(ast, ctx) {
            return true;
        }
        c = ast.decls.get(ctx).ctx();
    }
    false
}

/// True if the namespace-scope function or variable `d` has internal
/// linkage by its own declaration ([basic.link]/3): `static`, or a
/// non-`extern`/non-`inline` `const` variable.
fn internal_by_storage(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    let n = ast.decls.get(first);
    if !is_file_context(ast, first) {
        return false;
    }
    match n.kind() {
        DeclKind::Function => {
            ast.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static)
        }
        DeclKind::Var | DeclKind::VarTemplateSpecialization => {
            let Some(v) = ast.decls.var(first) else { return false };
            if v.spec.storage() == VarStorage::Static {
                return true;
            }
            let q = ast.types.canonical(n.ty().unwrap_or(QualType::INT));
            let is_const = ast.types.effective_quals(q).is_const() || const_array(ast, q);
            is_const
                && !v.spec.is_inline()
                && v.spec.storage() != VarStorage::Extern
                && !is_extern_c(ast, first)
                && !matches!(v.templ.get(), Some((TemplLinkKind::Specialization, _)))
        }
        _ => false,
    }
}

/// True if `d`'s semantic context is a namespace (or the translation
/// unit), looking through linkage specifications.
fn is_file_context(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        match ast.decls.get(ctx).kind() {
            DeclKind::TranslationUnit | DeclKind::Namespace => return true,
            DeclKind::LinkageSpec | DeclKind::Export => c = ast.decls.get(ctx).ctx(),
            _ => return false,
        }
    }
    true
}

/// True if `d` (a function, member function, variable or static data
/// member) has internal linkage ([basic.link]/4): `static` or `const` at
/// namespace scope, a member of an unnamed namespace (or of a class in
/// one), or a member of a local class of a function with internal
/// linkage.
pub fn has_internal_linkage(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    if internal_by_storage(ast, first) || in_anon_ns(ast, first) {
        return true;
    }
    // A member of a local class takes the linkage of the function.
    let mut c = ast.decls.get(first).ctx();
    while let Some(ctx) = c {
        let k = ast.decls.get(ctx).kind();
        if is_function_kind(k) {
            return has_internal_linkage(ast, ctx);
        }
        if !is_record_kind(k) {
            break;
        }
        c = ast.decls.get(ctx).ctx();
    }
    false
}

fn const_array(ast: &Ast, q: QualType) -> bool {
    let mut q = q;
    while let Some(e) = ast.types.element(q) {
        q = ast.types.canonical(e);
        if ast.types.effective_quals(q).is_const() {
            return true;
        }
    }
    false
}

/// The spelling of `d`'s name (empty for unnamed entities).
pub fn decl_name(ast: &Ast, d: DeclId) -> String {
    let n = ast.decls.get(d);
    if let DeclNameView::Identifier(s) = ast.names.view(n.name()) {
        ast.symbols().as_str(s).to_owned()
    } else {
        if let Some(td) = ast.decls.typedef_for_anon(d) {
            return decl_name(ast, td);
        }
        ast.names.simple_spelling(n.name(), ast.symbols()).unwrap_or_default()
    }
}

/// True if the function or variable `d` is emitted under its plain
/// source name: `main`, `extern "C"` entities, and variables at global
/// scope.
pub fn is_unmangled(ast: &Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    if is_extern_c(ast, d) {
        return true;
    }
    match n.kind() {
        DeclKind::Function => decl_name(ast, d) == "main" && in_global_ns(ast, d),
        // Only variables with internal linkage are mangled at global scope
        // (clang `shouldMangleCXXName`): `static int h;` is `_ZL1h`.
        // A variable template specialization is always mangled
        // (`_Z2viILi3EE`).
        DeclKind::Var => {
            in_global_ns(ast, d)
                && !is_local(ast, d)
                && !has_internal_linkage(ast, d)
                && ast.spec_info(d).is_none()
        }
        _ => false,
    }
}

fn in_global_ns(ast: &Ast, d: DeclId) -> bool {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        match ast.decls.get(ctx).kind() {
            DeclKind::TranslationUnit => return true,
            DeclKind::LinkageSpec | DeclKind::Export => c = ast.decls.get(ctx).ctx(),
            _ => return false,
        }
    }
    true
}

fn is_local(ast: &Ast, d: DeclId) -> bool {
    enclosing_function(ast, d).is_some()
}

/// The function whose body declares `d` (directly or in a local class),
/// if any.
pub fn enclosing_function(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        if is_function_kind(ast.decls.get(ctx).kind()) {
            return Some(ctx);
        }
        c = ast.decls.get(ctx).ctx();
    }
    None
}

/// The GNU asm label of `d`'s entity (recorded on its first declaration),
/// which replaces its linkage name. clang's `MangleContext::mangleName`
/// prefixes LLVM's `\x01` "no global prefix" marker when the target's
/// user-label prefix is non-empty (Mach-O's `_`); every kiln target is
/// ELF, whose prefix is empty, so the label is the symbol verbatim (a
/// `\x01` the user wrote stays: LLVM strips it when emitting).
pub fn asm_label(ast: &Ast, d: DeclId) -> Option<String> {
    let l = ast.decls.cold.asm_label.get(ast.first(d))?;
    Some(String::from_utf8_lossy(ast.strings.bytes(*l)).into_owned())
}

/// The linkage name of a function, variable or static data member (a
/// constructor or destructor gets its complete-object variant).
pub fn mangle_decl(ast: &Ast, d: DeclId) -> String {
    mangle_structor(ast, d, Structor::Complete)
}

/// The linkage name of `d` with constructor/destructor variant `v`
/// (ignored for other functions).
pub fn mangle_structor(ast: &Ast, d: DeclId, v: Structor) -> String {
    if let Some(l) = asm_label(ast, d) {
        return l;
    }
    if is_unmangled(ast, d) {
        return decl_name(ast, d);
    }
    let mut m = Mangler::new(ast, "_Z");
    m.encoding(d, v);
    m.out
}

/// The linkage name of template parameter object `d` ([temp.param]/8):
/// `_ZTA` and its value as a template argument.
pub fn mangle_param_object(ast: &Ast, d: DeclId) -> String {
    let mut m = Mangler::new(ast, "_ZTA");
    if let Some(v) = ast.templates.param_object_value(d) {
        let ty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
        m.value_arg(ast.types.canonical(ty).without_quals(), v);
    }
    m.out
}

/// The linkage name of a static local `d`, the `index`-th (0-based)
/// static local of its function with the same name.
pub fn mangle_static_local(ast: &Ast, d: DeclId, index: u32) -> String {
    if let Some(l) = asm_label(ast, d) {
        return l;
    }
    let mut s = mangle_decl(ast, d);
    push_discriminator(&mut s, index);
    s
}

/// <discriminator> := _ <number> (< 10) | __ <number> _ ; the first
/// occurrence has none, the second is number 0.
fn push_discriminator(s: &mut String, index: u32) {
    use std::fmt::Write as _;
    if let Some(n) = index.checked_sub(1) {
        if n < 10 {
            let _ = write!(s, "_{n}");
        } else {
            let _ = write!(s, "__{n}_");
        }
    }
}

/// The guard variable of static-storage variable `d` (`_ZGV <name>`);
/// `index` is the static-local discriminator index (0 elsewhere).
pub fn mangle_guard(ast: &Ast, d: DeclId, index: u32) -> String {
    let name = mangle_static_local(ast, d, index);
    match name.strip_prefix("_Z") {
        Some(rest) => format!("_ZGV{rest}"),
        None => format!("_ZGV{}{name}", name.len()),
    }
}

/// The reference temporary `index` (clang's mangling number) extended by
/// variable `d`: `_ZGR <name> [<seq-id>] _` ([abi:mangling]
/// <special-name>).
pub fn mangle_ref_temporary(ast: &Ast, d: DeclId, local_index: u32, index: u32) -> String {
    let name = mangle_static_local(ast, d, local_index);
    let mut s = match name.strip_prefix("_Z") {
        Some(rest) => format!("_ZGR{rest}"),
        None => format!("_ZGR{}{name}", name.len()),
    };
    if let Some(n) = index.checked_sub(1) {
        s.push_str(&base36(n as usize));
    }
    s.push('_');
    s
}

/// The <type> mangling of `q` on its own (no substitutions shared with
/// an enclosing symbol).
pub fn mangle_type(ast: &Ast, q: QualType) -> String {
    let mut m = Mangler::new(ast, "");
    m.ty(q);
    m.out
}

/// `<special-name>` of a type: `prefix` (`_ZTI` / `_ZTS`) followed by its
/// <type> (WP-080).
pub fn mangle_rtti(ast: &Ast, q: QualType, prefix: &str) -> String {
    let mut m = Mangler::new(ast, prefix);
    m.ty(q);
    m.out
}

/// `<special-name>` of a class: `prefix` followed by the class's <type>
/// (`_ZTV` vtable, `_ZTT` VTT, `_ZTI` `type_info`, `_ZTS` `type_info` name;
/// WP-080).
pub fn mangle_class_special(ast: &Ast, prefix: &str, class: DeclId) -> String {
    let mut m = Mangler::new(ast, prefix);
    m.tag(class);
    m.out
}

/// The construction vtable of base `base` at byte `offset` in
/// `most_derived` (`_ZTC <type> <offset> _ <type>`, one substitution
/// table for both types, as clang's `mangleCXXCtorVTable`).
pub fn mangle_ctor_vtable(ast: &Ast, most_derived: DeclId, offset: i64, base: DeclId) -> String {
    use std::fmt::Write as _;
    let mut m = Mangler::new(ast, "_ZTC");
    m.tag(most_derived);
    let _ = write!(m.out, "{offset}_");
    m.tag(base);
    m.out
}

/// A thunk of `method` (variant `v` for a destructor): `this` adjustment
/// `this` = (non-virtual, vcall offset offset) and return adjustment
/// `ret` = (non-virtual, vbase offset offset) (Itanium §5.1.4
/// <special-name> `T <call-offset> <encoding>` / `Tc <call-offset>
/// <call-offset> <encoding>`).
pub fn mangle_thunk(
    ast: &Ast,
    method: DeclId,
    v: Structor,
    this: (i64, i64),
    ret: (i64, i64),
) -> String {
    let mut m = Mangler::new(ast, "_ZT");
    let covariant = ret != (0, 0);
    if covariant {
        m.out.push('c');
    }
    push_call_offset(&mut m.out, this);
    if covariant {
        push_call_offset(&mut m.out, ret);
    }
    m.encoding(method, v);
    m.out
}

/// <call-offset> ::= h <nv-offset> _ | v <offset number> _ <virtual offset number> _
fn push_call_offset(s: &mut String, (nv, virt): (i64, i64)) {
    use std::fmt::Write as _;
    let num = |n: i64| if n < 0 { format!("n{}", n.unsigned_abs()) } else { n.to_string() };
    if virt == 0 {
        let _ = write!(s, "h{}_", num(nv));
    } else {
        let _ = write!(s, "v{}_{}_", num(nv), num(virt));
    }
}

/// The Itanium mangler state for one symbol.
struct Mangler<'a> {
    ast: &'a Ast,
    out: String,
    subs: Vec<Sub>,
}

impl<'a> Mangler<'a> {
    fn new(ast: &'a Ast, prefix: &str) -> Self {
        Mangler { ast, out: String::from(prefix), subs: Vec::new() }
    }

    /// The semantic parent of `d`, looking through linkage
    /// specifications and `export`.
    fn parent(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(ctx) = c {
            match self.ast.decls.get(ctx).kind() {
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(ctx).ctx(),
                _ => return Some(ctx),
            }
        }
        None
    }

    /// The template and arguments of `d` when it is a class, function or
    /// variable template specialization.
    fn spec_of(&self, d: DeclId) -> Option<(DeclId, ArgListId)> {
        let (_, s) = self.ast.spec_info(d)?;
        Some((s.template, s.args))
    }

    fn is_std(&self, ns: DeclId) -> bool {
        self.ast.decls.get(ns).kind() == DeclKind::Namespace
            && decl_name(self.ast, ns) == "std"
            && self
                .parent(ns)
                .is_none_or(|p| self.ast.decls.get(p).kind() == DeclKind::TranslationUnit)
    }

    // ---- encodings -----------------------------------------------------

    /// <encoding> ::= <name> <bare-function-type> | <name>
    fn encoding(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        let is_fn = is_function_kind(n.kind());
        if is_fn && (is_extern_c(ast, d) || (decl_name(ast, d) == "main" && in_global_ns(ast, d))) {
            // Unmangled functions appear in local names by source name only.
            self.source_name(&decl_name(ast, d));
            return;
        }
        self.name(d, v);
        if is_fn {
            self.bare_function_type(d);
        }
    }

    /// <bare-function-type> of function `d`; a function template
    /// specialization mangles its template's signature with the return
    /// type first.
    fn bare_function_type(&mut self, d: DeclId) {
        let ast = self.ast;
        let kind = ast.decls.get(d).kind();
        let mut src = d;
        let mut with_ret = false;
        if let Some((t, _)) = self.spec_of(d) {
            if let Some(p) = ast.template_info(t).and_then(|i| i.pattern) {
                src = p;
            }
            with_ret = !matches!(
                kind,
                DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXConversion
            );
        }
        let Some(ty) = ast.decls.get(src).ty() else { return };
        let ty = ast.types.canonical(ty);
        let Some(p) = ast.types.proto(ty).copied() else { return };
        if with_ret {
            self.ty(p.ret());
        }
        let params: Vec<QualType> = ast.types.params(&p).to_vec();
        if params.is_empty() && !p.variadic() {
            self.out.push('v');
        }
        for q in params {
            self.ty(self.param_type(q));
        }
        if p.variadic() {
            self.out.push('z');
        }
    }

    /// The signature type of a parameter: top-level cv-qualifiers
    /// dropped.
    fn param_type(&self, q: QualType) -> QualType {
        let q = self.ast.types.canonical(q);
        if self.ast.types.element(q).is_some() {
            return q;
        }
        q.without_quals()
    }

    // ---- names ---------------------------------------------------------

    /// <name> of an entity.
    fn name(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        if let Some(f) = enclosing_function(ast, d) {
            // <local-name> := Z <function encoding> E <entity name>
            self.out.push('Z');
            self.encoding(f, Structor::Complete);
            self.out.push('E');
            self.name_in(d, v, Some(f));
            return;
        }
        self.name_in(d, v, None);
    }

    /// The name of `d` relative to `stop` (the enclosing function of a
    /// local entity, or the translation unit).
    fn name_in(&mut self, d: DeclId, v: Structor, stop: Option<DeclId>) {
        let parent = self.parent(d);
        let at_top = parent.is_none_or(|p| {
            Some(p) == stop || self.ast.decls.get(p).kind() == DeclKind::TranslationUnit
        });
        let spec = self.spec_of(d);
        if at_top || parent.is_some_and(|p| self.is_std(p)) {
            // <unscoped-name> / <unscoped-template-name> <template-args>
            let std = !at_top;
            if let Some((t, args)) = spec {
                self.unscoped_template(t, std);
                self.template_args_of(t, args);
            } else {
                if std {
                    self.out.push_str("St");
                }
                self.l_prefix(d);
                self.unqualified(d, v);
            }
            return;
        }
        // <nested-name>
        self.out.push('N');
        self.method_quals(d);
        let p = parent.expect("nested name has a parent");
        if let Some((t, args)) = spec {
            self.template_prefix(t, stop);
            self.template_args_of(t, args);
        } else {
            self.prefix(p, stop);
            self.l_prefix(d);
            self.unqualified(d, v);
        }
        self.out.push('E');
    }

    /// `L` for an internal-linkage name at namespace scope outside an
    /// unnamed namespace (clang `isInternalLinkageDecl`).
    fn l_prefix(&mut self, d: DeclId) {
        if internal_by_storage(self.ast, d) && !in_anon_ns(self.ast, d) {
            self.out.push('L');
        }
    }

    /// The cv- and ref-qualifiers (and `H` for an explicit object
    /// parameter) of a member function's nested name.
    fn method_quals(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if !matches!(n.kind(), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            return;
        }
        if ast.decls.function(d).is_some_and(|f| f.spec.has_explicit_object()) {
            self.out.push('H');
            return;
        }
        let Some(p) = n.ty().and_then(|t| ast.types.proto(ast.types.canonical(t))) else { return };
        let info = p.info();
        let q = info.this_quals();
        if q.is_restrict() {
            self.out.push('r');
        }
        if q.is_volatile() {
            self.out.push('V');
        }
        if q.is_const() {
            self.out.push('K');
        }
        match info.ref_qual() {
            RefQual::LValue => self.out.push('R'),
            RefQual::RValue => self.out.push('O'),
            RefQual::None => {}
        }
    }

    /// <prefix> for context `dc` (a namespace or class), using and
    /// recording substitutions; nothing for the translation unit or
    /// `stop`.
    fn prefix(&mut self, dc: DeclId, stop: Option<DeclId>) {
        let ast = self.ast;
        let k = ast.decls.get(dc).kind();
        if Some(dc) == stop || k == DeclKind::TranslationUnit {
            return;
        }
        if matches!(k, DeclKind::LinkageSpec | DeclKind::Export) {
            if let Some(p) = self.parent(dc) {
                self.prefix(p, stop);
            }
            return;
        }
        if self.is_std(dc) {
            self.out.push_str("St");
            return;
        }
        let key = Sub::Decl(ast.first(dc));
        if self.try_sub(key) {
            return;
        }
        if let Some((t, args)) = self.spec_of(dc) {
            self.template_prefix(t, stop);
            self.template_args(args);
        } else {
            if let Some(p) = self.parent(dc) {
                self.prefix(p, stop);
            }
            self.unqualified(dc, Structor::Complete);
        }
        self.subs.push(key);
    }

    /// <template-prefix> of template `t` within a nested name.
    fn template_prefix(&mut self, t: DeclId, stop: Option<DeclId>) {
        let key = Sub::Template(self.ast.first(t));
        if self.try_sub(key) {
            return;
        }
        if let Some(p) = self.parent(t) {
            self.prefix(p, stop);
        }
        self.unqualified(t, Structor::Complete);
        self.subs.push(key);
    }

    /// <unscoped-template-name> of template `t` (`St`-prefixed in `std`).
    fn unscoped_template(&mut self, t: DeclId, std: bool) {
        let key = Sub::Template(self.ast.first(t));
        if self.try_sub(key) {
            return;
        }
        if std {
            self.out.push_str("St");
        }
        self.unqualified(t, Structor::Complete);
        self.subs.push(key);
    }

    /// <unqualified-name>: operator, constructor, destructor, conversion
    /// or literal-operator names, else the <source-name>.
    fn unqualified(&mut self, d: DeclId, v: Structor) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        match ast.names.view(n.name()) {
            DeclNameView::Operator(op) => {
                let explicit = n
                    .ty()
                    .and_then(|t| ast.types.proto(ast.types.canonical(t)))
                    .map_or(0, |p| ast.types.params(p).len());
                // The implicit object parameter counts towards the arity
                // (static member operators are arity-independent).
                let implicit = n.kind() == DeclKind::CXXMethod
                    && ast.decls.function(d).is_some_and(|f| {
                        f.spec.storage() != FnStorage::Static && !f.spec.has_explicit_object()
                    });
                self.out.push_str(operator_code(op, explicit + usize::from(implicit)));
            }
            DeclNameView::Constructor(_) => {
                self.out.push_str(if v == Structor::Base { "C2" } else { "C1" });
            }
            DeclNameView::Destructor(_) => self.out.push_str(match v {
                Structor::Complete => "D1",
                Structor::Base => "D2",
                Structor::Deleting => "D0",
            }),
            DeclNameView::Conversion(q) => {
                self.out.push_str("cv");
                self.ty(q);
            }
            DeclNameView::LiteralOperator(s) => {
                self.out.push_str("li");
                let s = ast.symbols().as_str(s).to_owned();
                self.source_name(&s);
            }
            _ => {
                if n.kind() == DeclKind::Namespace && is_anon_ns(ast, d) {
                    self.source_name("_GLOBAL__N_1");
                    return;
                }
                let nm = decl_name(ast, d);
                if nm.is_empty() {
                    // An unnamed class without a typedef name for linkage.
                    self.out.push_str("Ut_");
                } else {
                    self.source_name(&nm);
                }
            }
        }
    }

    fn source_name(&mut self, s: &str) {
        self.out.push_str(&s.len().to_string());
        self.out.push_str(s);
    }

    /// Emits a back-reference if `s` is a recorded candidate.
    fn try_sub(&mut self, s: Sub) -> bool {
        match self.subs.iter().position(|&x| x == s) {
            Some(k) => {
                self.sub_ref(k);
                true
            }
            None => false,
        }
    }

    fn sub_ref(&mut self, k: usize) {
        if k == 0 {
            self.out.push_str("S_");
        } else {
            self.out.push('S');
            self.out.push_str(&base36(k - 1));
            self.out.push('_');
        }
    }

    // ---- template arguments ---------------------------------------------

    /// The template arguments `list` of a specialization of `t`. A
    /// function template can be overloaded on its template-head, so an
    /// argument whose natural parameter differs from the actual one is
    /// prefixed by the parameter (clang 18+ `<template-param-decl>`,
    /// cxx-abi issue 47): `Tn <type>` for a non-type parameter of
    /// deduced or dependent type, `Tp` for a pack.
    fn template_args_of(&mut self, t: DeclId, list: ArgListId) {
        let ast = self.ast;
        let overloadable = ast.decls.get(t).kind() == DeclKind::FunctionTemplate
            && self
                .parent(t)
                .is_none_or(|c| ast.decls.record(c).is_none_or(|r| r.lambda.is_none()));
        let params: Vec<DeclId> = match ast.template_info(t) {
            Some(info) if overloadable => {
                let l = *ast.templates.param_list(info.params);
                ast.lists.iter::<DeclId>(l.params).collect()
            }
            _ => return self.template_args(list),
        };
        self.out.push('I');
        let args: Vec<_> = ast.types.args(list).to_vec();
        for (i, a) in args.into_iter().enumerate() {
            let a = a.view();
            let p = params.get(i).or_else(|| {
                params.last().filter(|&&p| ast.template_parm(p).is_some_and(|x| x.pack))
            });
            if let Some(&p) = p
                && self.needs_param_decl(p, a)
            {
                self.template_param_decl(p);
            }
            self.template_arg(a);
        }
        self.out.push('E');
    }

    /// clang `needToMangleTemplateParam`.
    fn needs_param_decl(&self, p: DeclId, a: ArgView) -> bool {
        let ast = self.ast;
        match ast.decls.get(p).kind() {
            // A constrained type parameter's `Tk <type-constraint>` is not
            // mangled yet.
            DeclKind::NonTypeTemplateParm => {}
            _ => return false,
        }
        if let ArgView::Pack(l) = a {
            match ast.types.args(l).first() {
                None => return true,
                Some(x) => return self.needs_param_decl(p, x.view()),
            }
        }
        ast.decls.get(p).ty().is_some_and(|t| {
            ast.types.flags(t.ty()).intersects(
                kiln_ast::types::TypeFlags::DEPENDENT
                    | kiln_ast::types::TypeFlags::INSTANTIATION_DEPENDENT
                    | kiln_ast::types::TypeFlags::UNDEDUCED,
            )
        })
    }

    /// <template-param-decl> of non-type parameter `p`: `[Tp] Tn <type>`.
    fn template_param_decl(&mut self, p: DeclId) {
        let ast = self.ast;
        let mut ty = ast.decls.get(p).ty().unwrap_or(QualType::INT);
        if ast.template_parm(p).is_some_and(|x| x.pack) {
            self.out.push_str("Tp");
            if let TypeView::PackExpansion { pattern, .. } = ast.types.view(ty.ty()) {
                ty = pattern;
            }
        }
        self.out.push_str("Tn");
        self.ty(ty);
    }

    fn template_args(&mut self, list: ArgListId) {
        self.out.push('I');
        let args: Vec<_> = self.ast.types.args(list).to_vec();
        for a in args {
            self.template_arg(a.view());
        }
        self.out.push('E');
    }

    fn template_arg(&mut self, a: ArgView) {
        match a {
            ArgView::Type(q) => self.ty(q),
            ArgView::Value { ty, value } => self.value_arg(ty, value),
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                if matches!(a, ArgView::TemplateExpansion { .. }) {
                    self.out.push_str("Dp");
                }
                self.template_name(n);
            }
            ArgView::Pack(l) => {
                self.out.push('J');
                let args: Vec<_> = self.ast.types.args(l).to_vec();
                for x in args {
                    self.template_arg(x.view());
                }
                self.out.push('E');
            }
            ArgView::Null | ArgView::Expr { .. } | ArgView::Profile(_) => {
                self.out.push_str("XLi0EE");
            }
        }
    }

    /// A template template argument.
    fn template_name(&mut self, n: TemplateNameId) {
        let ast = self.ast;
        let n = ast.types.canonical_template_name(n);
        match ast.types.template_name_view(n) {
            TemplateName::Template(t) => {
                let key = Sub::Template(ast.first(t));
                if self.try_sub(key) {
                    return;
                }
                let parent = self.parent(t);
                let top =
                    parent.is_none_or(|p| ast.decls.get(p).kind() == DeclKind::TranslationUnit);
                if top {
                    self.unqualified(t, Structor::Complete);
                } else if parent.is_some_and(|p| self.is_std(p)) {
                    self.out.push_str("St");
                    self.unqualified(t, Structor::Complete);
                } else {
                    self.out.push('N');
                    if let Some(p) = parent {
                        self.prefix(p, None);
                    }
                    self.unqualified(t, Structor::Complete);
                    self.out.push('E');
                }
                self.subs.push(key);
            }
            TemplateName::TemplateTemplateParm { decl, .. } => {
                let idx = ast.template_parm(decl).map_or(0, |p| u32::from(p.index));
                self.template_param(idx);
            }
            _ => self.out.push_str("u7unknown"),
        }
    }

    /// A non-type template argument value: `L <type> <value> E`, or
    /// `L <mangled-name> E` for a reference to an entity.
    fn value_arg(&mut self, ty: QualType, value: ConstValueId) {
        self.value_in_arg(ty, value, true);
    }

    /// clang `mangleValueInTemplateArg`: value `value` of type `ty`; at
    /// the top level of a template argument (`top`) a non-primary
    /// expression is wrapped in `X…E`.
    fn value_in_arg(&mut self, ty: QualType, value: ConstValueId, top: bool) {
        let ty = self.value_type(ty);
        match self.ast.consts.get(value) {
            ConstValue::Aggregate(img) => self.image_value(&img, ty, 0, top),
            ConstValue::Scalar(s) => self.scalar_value(ty, &s, top),
        }
    }

    /// A value's type with its top-level cv-qualifiers dropped ("to match
    /// GCC").
    fn value_type(&self, ty: QualType) -> QualType {
        self.ast.types.canonical(ty).without_quals()
    }

    fn scalar_value(&mut self, ty: QualType, s: &Scalar<'_>, top: bool) {
        match *s {
            Scalar::Int { signed, bits, .. } => self.int_literal(ty, signed, &bits),
            Scalar::Bool { value, .. } => {
                self.out.push('L');
                self.ty(ty);
                self.out.push_str(if value { "1E" } else { "0E" });
            }
            Scalar::Float { kind, bits, .. } => self.float_literal(ty, kind.bits(), bits),
            Scalar::Null { .. } => self.null_literal(ty),
            Scalar::Pointer { base, offset, .. } => self.pointer_value(ty, base, offset, top),
            Scalar::MemberPtr { member, path, .. } => {
                // `[X] [mc <type>] adL <member> E [E] [E]`.
                if top {
                    self.out.push('X');
                }
                if !path.is_empty() {
                    self.out.push_str("mc");
                    self.ty(ty);
                }
                self.out.push_str("adL");
                self.entity(member);
                self.out.push('E');
                if !path.is_empty() {
                    // The path adjustment (nonzero only for a non-primary
                    // base) is not computed.
                    self.out.push('E');
                }
                if top {
                    self.out.push('E');
                }
            }
            Scalar::Reflection { .. } => self.out.push_str("XLi0EE"),
        }
    }

    /// `L <type> <number> E` (`n` for negative; `0`/`1` for `bool`).
    fn int_literal(&mut self, ty: QualType, signed: bool, bits: &kiln_ast::consts::IntBits<'_>) {
        self.out.push('L');
        self.ty(ty);
        let mut v = String::new();
        bits.write_decimal(signed, &mut v);
        if let Some(mag) = v.strip_prefix('-') {
            self.out.push('n');
            self.out.push_str(mag);
        } else {
            self.out.push_str(&v);
        }
        self.out.push('E');
    }

    /// `L <type> <hex> E`: the value bits as fixed-width lowercase hex.
    fn float_literal(&mut self, ty: QualType, width: u32, bits: u128) {
        self.out.push('L');
        self.ty(ty);
        let n = width.div_ceil(4);
        for i in (0..n).rev() {
            let d = (bits >> (4 * i)) & 0xf;
            self.out.push(char::from_digit(d as u32, 16).unwrap_or('0'));
        }
        self.out.push('E');
    }

    /// `L <type> 0 E` (clang `mangleNullPointer`).
    fn null_literal(&mut self, ty: QualType) {
        self.out.push('L');
        self.ty(ty);
        self.out.push_str("0E");
    }

    /// `_Z <encoding>` of `d`, or `_ZTA <value>` for a template parameter
    /// object, sharing this symbol's substitutions.
    fn entity(&mut self, d: DeclId) {
        let ast = self.ast;
        if ast.decls.get(d).kind() == DeclKind::TemplateParamObject {
            self.out.push_str("_ZTA");
            if let Some(v) = ast.templates.param_object_value(d) {
                let ty = ast.decls.get(d).ty().unwrap_or(QualType::INT);
                self.value_in_arg(ty, v, true);
            }
            return;
        }
        self.out.push_str("_Z");
        self.encoding(d, Structor::Complete);
    }

    /// A pointer or reference value (clang's `APValue::LValue` case):
    /// `[X] [ad] L <entity> E [E]` for a whole object, else the subobject
    /// form `[X] [ad] so <referent type> L <entity> E [<offset>]
    /// <union-selector>* [p] E [E]`.
    fn pointer_value(&mut self, ty: QualType, base: PtrBase, offset: u64, top: bool) {
        let ast = self.ast;
        let is_ref =
            matches!(ast.types.canonical_view(ty), TypeView::LValueRef(_) | TypeView::RValueRef(_));
        let pointee = ast.types.pointee(ty);
        let d = match base {
            PtrBase::Decl(d) | PtrBase::Function(d) => d,
            PtrBase::TypeInfo(t) => {
                self.out.push_str(if top { "X" } else { "" });
                self.out.push_str(if is_ref { "ti" } else { "adti" });
                self.ty(t);
                self.out.push_str(if top { "E" } else { "" });
                return;
            }
            // Temporaries, string literals and frozen objects are not
            // template arguments.
            _ => {
                self.out.push_str("XLi0EE");
                return;
            }
        };
        let path = match base {
            PtrBase::Decl(_) => self.lvalue_path(ast.decls.get(d).ty(), offset, pointee),
            _ => LvaluePath::default(),
        };
        let wrap = top && (!is_ref || path.nonempty || path.past_end);
        if wrap {
            self.out.push('X');
        }
        if !is_ref {
            self.out.push_str("ad");
        }
        if path.nonempty || path.past_end {
            self.out.push_str("so");
            self.ty(pointee.unwrap_or(ty));
            self.out.push('L');
            self.entity(d);
            self.out.push('E');
            if offset != 0 {
                self.out.push_str(&offset.to_string());
            }
            self.out.push_str(&path.selectors);
            if path.past_end {
                self.out.push('p');
            }
            self.out.push('E');
        } else {
            self.out.push('L');
            self.entity(d);
            self.out.push('E');
        }
        if wrap {
            self.out.push('E');
        }
    }

    /// The lvalue path from an object of type `obj` to the subobject at
    /// byte `offset` of type `pointee` (kiln pointers are `(base,
    /// offset)`; clang keeps the path).
    fn lvalue_path(
        &self,
        obj: Option<QualType>,
        offset: u64,
        pointee: Option<QualType>,
    ) -> LvaluePath {
        let ast = self.ast;
        let same = |a: QualType, b: QualType| {
            ast.types.canonical(a).without_quals() == ast.types.canonical(b).without_quals()
        };
        let mut r = LvaluePath::default();
        let Some(mut cur) = obj else {
            r.past_end = offset != 0;
            return r;
        };
        let mut off = offset;
        // Bounded by the nesting depth of the type.
        for _ in 0..4096 {
            if off == 0 && pointee.is_none_or(|p| same(p, cur)) {
                break;
            }
            match ast.shape(cur) {
                Shape::Array { elem, stride, len } if stride != 0 => {
                    let i = off / stride;
                    r.nonempty = true;
                    if i >= len {
                        r.past_end = true;
                        break;
                    }
                    off -= i * stride;
                    cur = elem;
                }
                Shape::Record { members, union } => {
                    let inside = |m: &&Member| {
                        let at = m.offset_bits / 8;
                        m.bit_width.is_none()
                            && m.offset_bits.is_multiple_of(8)
                            && at <= off
                            && (off < at + m.size || (off == at && m.size == 0))
                    };
                    let hit = members
                        .iter()
                        .filter(inside)
                        .find(|m| {
                            off == m.offset_bits / 8 && pointee.is_some_and(|p| same(p, m.ty))
                        })
                        .or_else(|| members.iter().find(inside));
                    let Some(m) = hit else { break };
                    r.nonempty = true;
                    if union {
                        // <union-selector> ::= _ [<number>]
                        let idx = members
                            .iter()
                            .filter(|x| x.field.is_some())
                            .position(|x| x.field == m.field)
                            .unwrap_or(0);
                        r.selectors.push('_');
                        if idx > 0 {
                            r.selectors.push_str(&(idx - 1).to_string());
                        }
                    }
                    off -= m.offset_bits / 8;
                    cur = m.ty;
                }
                _ => {
                    // Past the end of a complete scalar object.
                    r.past_end = !r.nonempty && offset != 0;
                    break;
                }
            }
        }
        r
    }

    /// The pointer-like value relocated at byte `at` of `img`, for a
    /// scalar type.
    fn reloc_at(img: &ByteImage<'_>, at: u64) -> Option<ConstValueId> {
        img.relocs.binary_search_by_key(&at, |r| u64::from(r.at)).ok().map(|i| img.relocs[i].value)
    }

    /// The object of type `ty` at byte `at` of aggregate image `img`
    /// (clang's `Struct`, `Union` and `Array` cases; trailing
    /// zero-initialized elements are dropped).
    fn image_value(&mut self, img: &ByteImage<'_>, ty: QualType, at: u64, top: bool) {
        let ty = self.value_type(ty);
        let shape = self.ast.shape(ty);
        let scalar = !matches!(shape, Shape::Array { .. } | Shape::Record { .. });
        if scalar && let Some(v) = Self::reloc_at(img, at) {
            if let ConstValue::Scalar(s) = self.ast.consts.get(v) {
                self.scalar_value(ty, &s, top);
            }
            return;
        }
        match shape {
            Shape::Bool => {
                self.out.push('L');
                self.ty(ty);
                self.out.push_str(if read_le(img.bytes, at, 1) != 0 { "1E" } else { "0E" });
            }
            Shape::Int { width, signed, .. } => {
                let width = width.clamp(1, kiln_ast::consts::IntBits::MAX_WIDTH);
                let w = read_bits(img.bytes, at * 8, width);
                if let Some(bits) = kiln_ast::consts::IntBits::from_words(width, &w) {
                    self.int_literal(ty, signed, &bits);
                }
            }
            Shape::Float(kind) => {
                let bits = read_le(img.bytes, at, kind.value_bytes());
                self.float_literal(ty, kind.bits(), bits);
            }
            Shape::NullPtr | Shape::Pointer { .. } | Shape::MemberPointer => self.null_literal(ty),
            Shape::Unknown | Shape::Reflection => {
                self.out.push('L');
                self.ty(ty);
                self.out.push('E');
            }
            Shape::Array { elem, stride, len } => {
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                let mut n = len;
                while n > 0 && self.image_zero(img, elem, at + (n - 1) * stride) {
                    n -= 1;
                }
                for i in 0..n {
                    self.image_value(img, elem, at + i * stride, false);
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
            Shape::Record { members, union: false } => {
                let unnamed_bf = |m: &Member| m.bit_width.is_some() && m.name.is_empty();
                let mut fields: Vec<&Member> =
                    members.iter().filter(|m| m.field.is_some()).collect();
                while fields.last().is_some_and(|m| unnamed_bf(m) || self.member_zero(img, m, at)) {
                    fields.pop();
                }
                let mut bases: Vec<&Member> =
                    members.iter().filter(|m| m.field.is_none()).collect();
                if fields.is_empty() {
                    while bases.last().is_some_and(|m| self.member_zero(img, m, at)) {
                        bases.pop();
                    }
                }
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                for m in bases {
                    self.image_value(img, m.ty, at + m.offset_bits / 8, false);
                }
                for m in fields {
                    if !unnamed_bf(m) {
                        self.member_value(img, m, at);
                    }
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
            Shape::Record { members, union: true } => {
                let Some(m) = Self::active_member(img, &members, at) else {
                    self.out.push('L');
                    self.ty(ty);
                    self.out.push('E');
                    return;
                };
                self.out.push_str(if top { "Xtl" } else { "tl" });
                self.ty(ty);
                if !self.union_zero(img, &members, at) {
                    // <braced-expression> ::= di <field source-name> <expr>
                    self.out.push_str("di");
                    if let Some(n) = m.field.and_then(|f| self.union_init_name(f)) {
                        self.source_name(&n);
                    }
                    self.member_value(img, m, at);
                }
                self.out.push_str(if top { "EE" } else { "E" });
            }
        }
    }

    /// Member `m` of the record at byte `at` of `img`.
    fn member_value(&mut self, img: &ByteImage<'_>, m: &Member, at: u64) {
        match m.bit_width {
            Some(width) if width > 0 => {
                let ty = self.value_type(m.ty);
                let w = read_bits(img.bytes, at * 8 + m.offset_bits, width);
                if matches!(self.ast.shape(ty), Shape::Bool) {
                    self.out.push('L');
                    self.ty(ty);
                    self.out.push_str(if w[0] != 0 { "1E" } else { "0E" });
                } else {
                    let signed = matches!(self.ast.shape(ty), Shape::Int { signed: true, .. });
                    if let Some(bits) = kiln_ast::consts::IntBits::from_words(width, &w) {
                        self.int_literal(ty, signed, &bits);
                    }
                }
            }
            Some(_) => {
                self.out.push('L');
                self.ty(m.ty);
                self.out.push_str("0E");
            }
            None => self.image_value(img, m.ty, at + m.offset_bits / 8, false),
        }
    }

    /// The active member of the union at byte `at` of `img`.
    fn active_member<'m>(
        img: &ByteImage<'_>,
        members: &'m [Member],
        at: u64,
    ) -> Option<&'m Member> {
        img.unions
            .iter()
            .filter(|u| u64::from(u.at) == at)
            .find_map(|u| members.iter().find(|m| m.field == Some(u.field)))
    }

    /// The name a union initializer designates (clang `getUnionInitName`):
    /// the field's, or for an anonymous member the first named field of
    /// a depth-first walk.
    fn union_init_name(&self, f: DeclId) -> Option<String> {
        let ast = self.ast;
        let n = decl_name(ast, f);
        if !n.is_empty() {
            return Some(n);
        }
        if ast.decls.field(f).is_some_and(|fd| fd.bit_width.is_some()) {
            return None;
        }
        let ty = ast.decls.get(f).ty()?;
        let TypeView::Record(rd) = ast.types.canonical_view(ty) else { return None };
        let def = ast.definition(rd)?;
        ast.members(def)
            .filter(|&x| ast.decls.get(x).kind() == DeclKind::Field)
            .find_map(|x| self.union_init_name(x))
    }

    /// clang `isZeroInitialized` for the object of type `ty` at byte `at`.
    fn image_zero(&self, img: &ByteImage<'_>, ty: QualType, at: u64) -> bool {
        let shape = self.ast.shape(ty);
        let scalar = !matches!(shape, Shape::Array { .. } | Shape::Record { .. });
        if scalar && let Some(v) = Self::reloc_at(img, at) {
            return matches!(self.ast.consts.get(v), ConstValue::Scalar(Scalar::Null { .. }));
        }
        match shape {
            Shape::Bool => read_le(img.bytes, at, 1) == 0,
            Shape::Int { width, .. } => {
                read_bits(img.bytes, at * 8, width.max(1)).iter().all(|&w| w == 0)
            }
            Shape::Float(kind) => read_le(img.bytes, at, kind.value_bytes()) == 0,
            Shape::NullPtr | Shape::Pointer { .. } | Shape::MemberPointer => true,
            Shape::Unknown | Shape::Reflection => false,
            Shape::Array { elem, stride, len } => {
                (0..len).all(|i| self.image_zero(img, elem, at + i * stride))
            }
            Shape::Record { members, union: false } => members
                .iter()
                .filter(|m| !(m.bit_width.is_some() && m.name.is_empty()))
                .all(|m| self.member_zero(img, m, at)),
            Shape::Record { members, union: true } => self.union_zero(img, &members, at),
        }
    }

    /// Whether member `m` of the record at byte `at` is zero.
    fn member_zero(&self, img: &ByteImage<'_>, m: &Member, at: u64) -> bool {
        match m.bit_width {
            Some(width) => {
                read_bits(img.bytes, at * 8 + m.offset_bits, width.max(1)).iter().all(|&w| w == 0)
            }
            None => self.image_zero(img, m.ty, at + m.offset_bits / 8),
        }
    }

    /// A union is zero-initialized when its first named member is active
    /// and zero, or it has no named member.
    fn union_zero(&self, img: &ByteImage<'_>, members: &[Member], at: u64) -> bool {
        let first = members.iter().find(|m| !(m.bit_width.is_some() && m.name.is_empty()));
        let Some(first) = first else { return true };
        Self::active_member(img, members, at).is_some_and(|a| a.field == first.field)
            && self.member_zero(img, first, at)
    }

    fn template_param(&mut self, index: u32) {
        if index == 0 {
            self.out.push_str("T_");
        } else {
            self.out.push('T');
            self.out.push_str(&(index - 1).to_string());
            self.out.push('_');
        }
    }

    // ---- types -----------------------------------------------------------

    /// A tag type: its name (a candidate keyed by the declaration).
    fn tag(&mut self, d: DeclId) {
        let key = Sub::Decl(self.ast.first(d));
        if self.try_sub(key) {
            return;
        }
        self.name(d, Structor::Complete);
        self.subs.push(key);
    }

    fn ty(&mut self, q: QualType) {
        let ast = self.ast;
        let q = ast.types.canonical(q);
        let quals = ast.types.effective_quals(q);
        let is_array = ast.types.element(q).is_some();
        if !quals.is_empty() && !is_array {
            let key = Sub::Type(q);
            if self.try_sub(key) {
                return;
            }
            if quals.is_restrict() {
                self.out.push('r');
            }
            if quals.is_volatile() {
                self.out.push('V');
            }
            if quals.is_const() {
                self.out.push('K');
            }
            self.ty(q.without_quals());
            self.subs.push(key);
            return;
        }
        let view = ast.types.canonical_view(q);
        match view {
            TypeView::Builtin(b) => {
                self.out.push_str(builtin_code(b));
                return;
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                self.tag(d);
                return;
            }
            _ => {}
        }
        let key = Sub::Type(q);
        if self.try_sub(key) {
            return;
        }
        self.compound(q, view);
        self.subs.push(key);
    }

    /// A substitutable type other than a qualified type, builtin or tag.
    fn compound(&mut self, q: QualType, view: TypeView) {
        let ast = self.ast;
        match view {
            TypeView::Pointer(p) => {
                self.out.push('P');
                self.ty(p);
            }
            TypeView::LValueRef(p) => {
                self.out.push('R');
                self.ty(p);
            }
            TypeView::RValueRef(p) => {
                self.out.push('O');
                self.ty(p);
            }
            TypeView::ConstantArray(_) => {
                let n = ast.types.array_size(q).unwrap_or(0);
                self.out.push('A');
                self.out.push_str(&n.to_string());
                self.out.push('_');
                if let Some(e) = ast.types.element(q) {
                    self.ty(e);
                }
            }
            TypeView::IncompleteArray(_) => {
                self.out.push_str("A_");
                if let Some(e) = ast.types.element(q) {
                    self.ty(e);
                }
            }
            TypeView::Function(p) => self.function_type(q, &p),
            TypeView::MemberPointer { pointee, class } => self.member_pointer(pointee, class),
            TypeView::Complex(e) => {
                self.out.push('C');
                self.ty(e);
            }
            TypeView::Atomic(e) => {
                self.out.push_str("U7_Atomic");
                self.ty(e);
            }
            TypeView::Vector { elem, count, .. } => {
                self.out.push_str("Dv");
                self.out.push_str(&count.to_string());
                self.out.push('_');
                self.ty(elem);
            }
            TypeView::BitInt { signed, width } => {
                self.out.push_str(if signed { "DB" } else { "DU" });
                self.out.push_str(&width.to_string());
                self.out.push('_');
            }
            TypeView::TemplateTypeParm(d) => {
                let idx = ast.template_parm(d).map_or(0, |p| u32::from(p.index));
                self.template_param(idx);
            }
            TypeView::CanonTemplateTypeParm { index, .. } => self.template_param(index),
            TypeView::PackExpansion { pattern, .. } => {
                self.out.push_str("Dp");
                self.ty(pattern);
            }
            TypeView::Auto { keyword: kiln_ast::types::AutoKeyword::DecltypeAuto, .. } => {
                self.out.push_str("Dc");
            }
            TypeView::Auto { .. } => self.out.push_str("Da"),
            _ => self.out.push_str("u7unknown"),
        }
    }

    /// <pointer-to-member-type> ::= M <class type> <member type>; a
    /// member function's cv-qualifiers precede its function type.
    fn member_pointer(&mut self, pointee: QualType, class: kiln_ast::ids::TypeId) {
        let ast = self.ast;
        self.out.push('M');
        self.ty(QualType::unqual(class));
        let pointee = ast.types.canonical(pointee);
        if let Some(p) = ast.types.proto(pointee).copied() {
            let mq = p.info().this_quals();
            if mq.is_restrict() {
                self.out.push('r');
            }
            if mq.is_volatile() {
                self.out.push('V');
            }
            if mq.is_const() {
                self.out.push('K');
            }
            let fkey = Sub::Type(pointee);
            if !self.try_sub(fkey) {
                self.function_type(pointee, &p);
                self.subs.push(fkey);
            }
        } else {
            self.ty(pointee);
        }
    }

    /// <function-type> ::= [<CV>] [Do] F [Y] <return> <params> [<ref>] E
    /// (method cv-qualifiers are written by the member-pointer caller).
    fn function_type(&mut self, _q: QualType, p: &kiln_ast::types::FuncProto) {
        let ast = self.ast;
        if ast.types.exc_spec(p).is_nothrow() {
            self.out.push_str("Do");
        }
        self.out.push('F');
        self.ty(p.ret());
        let params: Vec<QualType> = ast.types.params(p).to_vec();
        if params.is_empty() && !p.variadic() {
            self.out.push('v');
        }
        for x in params {
            self.ty(self.param_type(x));
        }
        if p.variadic() {
            self.out.push('z');
        }
        match p.info().ref_qual() {
            RefQual::LValue => self.out.push('R'),
            RefQual::RValue => self.out.push('O'),
            RefQual::None => {}
        }
        self.out.push('E');
    }
}

/// The <builtin-type> code of `b`.
pub fn builtin_code(b: BuiltinType) -> &'static str {
    use BuiltinType as B;
    match b {
        B::Void => "v",
        B::Bool => "b",
        B::Char => "c",
        B::SChar => "a",
        B::UChar => "h",
        B::WChar => "w",
        B::Char8 => "Du",
        B::Char16 => "Ds",
        B::Char32 => "Di",
        B::Short => "s",
        B::UShort => "t",
        B::Int => "i",
        B::UInt => "j",
        B::Long => "l",
        B::ULong => "m",
        B::LongLong => "x",
        B::ULongLong => "y",
        B::Int128 => "n",
        B::UInt128 => "o",
        B::Float16 => "DF16_",
        B::BFloat16 => "DF16b",
        B::Float => "f",
        B::Double => "d",
        B::LongDouble => "e",
        B::Float128 => "g",
        B::Half => "Dh",
        B::NullPtr => "Dn",
        _ => "u7unknown",
    }
}

fn base36(mut n: usize) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if n == 0 {
        return "0".to_owned();
    }
    let mut v = Vec::new();
    while n > 0 {
        v.push(DIGITS[n % 36]);
        n /= 36;
    }
    v.reverse();
    String::from_utf8(v).unwrap_or_default()
}

/// The Itanium <operator-name> of `op` declared with `arity` operands
/// (the implicit object parameter included): `+ - * &` with one operand
/// are the unary forms.
pub fn operator_code(op: OverloadedOperator, arity: usize) -> &'static str {
    use OverloadedOperator as O;
    let unary = arity == 1;
    match op {
        O::New => "nw",
        O::ArrayNew => "na",
        O::Delete => "dl",
        O::ArrayDelete => "da",
        O::Plus if unary => "ps",
        O::Minus if unary => "ng",
        O::Star if unary => "de",
        O::Amp if unary => "ad",
        O::Plus => "pl",
        O::Minus => "mi",
        O::Star => "ml",
        O::Amp => "an",
        O::Slash => "dv",
        O::Percent => "rm",
        O::Caret => "eo",
        O::Pipe => "or",
        O::Tilde => "co",
        O::Exclaim => "nt",
        O::Equal => "aS",
        O::Less => "lt",
        O::Greater => "gt",
        O::PlusEqual => "pL",
        O::MinusEqual => "mI",
        O::StarEqual => "mL",
        O::SlashEqual => "dV",
        O::PercentEqual => "rM",
        O::CaretEqual => "eO",
        O::AmpEqual => "aN",
        O::PipeEqual => "oR",
        O::LessLess => "ls",
        O::GreaterGreater => "rs",
        O::LessLessEqual => "lS",
        O::GreaterGreaterEqual => "rS",
        O::EqualEqual => "eq",
        O::ExclaimEqual => "ne",
        O::LessEqual => "le",
        O::GreaterEqual => "ge",
        O::Spaceship => "ss",
        O::AmpAmp => "aa",
        O::PipePipe => "oo",
        O::PlusPlus => "pp",
        O::MinusMinus => "mm",
        O::Comma => "cm",
        O::ArrowStar => "pm",
        O::Arrow => "pt",
        O::Call => "cl",
        O::Subscript => "ix",
        O::Coawait => "aw",
    }
}

/// A subobject designation recovered from a byte offset.
#[derive(Default)]
struct LvaluePath {
    /// The path has at least one entry.
    nonempty: bool,
    /// One past the end of an array or complete object (`p`).
    past_end: bool,
    /// `<union-selector>*`.
    selectors: String,
}

/// `width` bits starting at bit `start` of little-endian `bytes`, as
/// normalized words; bits past the end read as zero.
fn read_bits(bytes: &[u8], start: u64, width: u32) -> Vec<u64> {
    let mut w = vec![0u64; kiln_ast::consts::IntBits::words_for(width)];
    for i in 0..u64::from(width) {
        let b = start + i;
        let set = bytes.get((b / 8) as usize).is_some_and(|&x| x >> (b % 8) & 1 != 0);
        if set {
            w[(i / 64) as usize] |= 1 << (i % 64);
        }
    }
    w
}

/// `n` (at most 16) little-endian bytes at `at`.
fn read_le(bytes: &[u8], at: u64, n: u32) -> u128 {
    (0..u64::from(n.min(16))).rev().fold(0u128, |acc, i| {
        (acc << 8) | u128::from(bytes.get((at + i) as usize).copied().unwrap_or(0))
    })
}
