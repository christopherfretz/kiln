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
use kiln_ast::consts::{ConstValue, PtrBase, Scalar};
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
        DeclKind::Var => in_global_ns(ast, d) && !is_local(ast, d) && !has_internal_linkage(ast, d),
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
                self.template_args(args);
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
            self.template_args(args);
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
        let ast = self.ast;
        match ast.consts.get(value) {
            ConstValue::Scalar(Scalar::Int { signed, bits, .. }) => {
                self.out.push('L');
                self.ty(ty);
                let v = if signed {
                    bits.to_i128().map(|v| (v < 0, v.unsigned_abs()))
                } else {
                    bits.to_u128().map(|v| (false, v))
                };
                if let Some((neg, mag)) = v {
                    if neg {
                        self.out.push('n');
                    }
                    self.out.push_str(&mag.to_string());
                }
                self.out.push('E');
            }
            ConstValue::Scalar(Scalar::Bool { value, .. }) => {
                self.out.push_str(if value { "Lb1E" } else { "Lb0E" });
            }
            ConstValue::Scalar(Scalar::Null { .. }) => {
                if matches!(ast.types.canonical_view(ty), TypeView::Builtin(BuiltinType::NullPtr)) {
                    self.out.push_str("LDn0E");
                } else {
                    self.out.push('L');
                    self.ty(ty);
                    self.out.push_str("0E");
                }
            }
            ConstValue::Scalar(Scalar::Pointer { base, offset: 0, .. }) => {
                let d = match base {
                    PtrBase::Decl(d) | PtrBase::Function(d) => Some(d),
                    _ => None,
                };
                let is_ref = matches!(
                    ast.types.canonical_view(ty),
                    TypeView::LValueRef(_) | TypeView::RValueRef(_)
                );
                let decayed = d.filter(|&d| {
                    !is_ref && ast.decls.get(d).ty().is_some_and(|t| ast.types.is_array(t))
                });
                let pointee = ast.types.pointee(ty);
                match d {
                    Some(d) if decayed.is_some() && pointee.is_some() => {
                        // `arr` decayed to `&arr[0]`: a subobject
                        // expression (`Xadso <elem> L_Z <arr> E E E`, clang
                        // `mangleTemplateArgValue` of an lvalue path).
                        self.out.push_str("Xadso");
                        self.ty(pointee.unwrap_or(ty).without_quals());
                        self.out.push_str("L_Z");
                        self.encoding(d, Structor::Complete);
                        self.out.push_str("EEE");
                    }
                    Some(d) => {
                        // The entity's encoding shares this symbol's
                        // substitutions.
                        self.out.push_str(if is_ref { "L_Z" } else { "XadL_Z" });
                        self.encoding(d, Structor::Complete);
                        self.out.push_str(if is_ref { "E" } else { "EE" });
                    }
                    None => self.out.push_str("XLi0EE"),
                }
            }
            ConstValue::Aggregate(_) => self.aggregate_arg(ty, value),
            _ => self.out.push_str("XLi0EE"),
        }
    }

    /// A class-type value (a template parameter object's value,
    /// [temp.param]/8). Itanium spells it `Xtl <type> <member exprs> EE`
    /// with each member's value; that walk needs the record layout, which
    /// the mangler does not have yet, so this seam (sema-templates.md
    /// §6.10) spells the image as its 8-byte little-endian words instead:
    /// deterministic and distinct for distinct byte images, but not
    /// clang's spelling (pointer members are not distinguished).
    fn aggregate_arg(&mut self, ty: QualType, value: ConstValueId) {
        self.out.push_str("Xtl");
        self.ty(ty);
        let bytes = self.ast.consts.bytes(value);
        let mut words: Vec<u64> = bytes
            .chunks(8)
            .map(|c| c.iter().rev().fold(0u64, |a, &b| (a << 8) | u64::from(b)))
            .collect();
        while words.last() == Some(&0) {
            words.pop();
        }
        for w in words {
            self.out.push_str("Ly");
            self.out.push_str(&w.to_string());
            self.out.push('E');
        }
        self.out.push_str("EE");
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
