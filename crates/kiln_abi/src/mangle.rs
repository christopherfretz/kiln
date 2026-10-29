//! Itanium C++ name mangling ([abi:mangling]) for the M2 subset: free
//! functions and variables in namespaces, internal-linkage names
//! (`_ZL`), static locals (`_ZZ...E`), and the types of the M2 type
//! system (builtins, pointers, references, arrays, function types,
//! classes and enumerations) with substitutions. See spec/codegen.md §3.

use kiln_ast::Ast;
use kiln_ast::decl::{DeclKind, FnStorage, LinkageLang, NamespaceData, VarStorage};
use kiln_ast::ids::{DeclId, QualType};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::{BuiltinType, TypeView};

/// A substitution candidate ([abi:mangling.compression]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sub {
    /// A class, enumeration or namespace prefix.
    Decl(DeclId),
    /// A (canonical, possibly qualified) type.
    Type(QualType),
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
            DeclKind::TranslationUnit | DeclKind::Namespace => {
                if n.kind() == DeclKind::TranslationUnit {
                    return false;
                }
            }
            _ => {}
        }
        c = n.ctx();
    }
    false
}

/// True if `ns` is an unnamed namespace.
fn is_anon_ns(ast: &Ast, ns: DeclId) -> bool {
    ast.decls.namespace(ns).is_some_and(|n| n.flags & NamespaceData::ANONYMOUS != 0)
}

/// True if `d` (a function or variable at namespace scope) has internal
/// linkage ([basic.link]/4): `static`, a non-`extern`/non-`inline`
/// `const` variable, or a member of an unnamed namespace.
pub fn has_internal_linkage(ast: &Ast, d: DeclId) -> bool {
    let first = ast.first(d);
    let n = ast.decls.get(first);
    match n.kind() {
        DeclKind::Function => {
            if ast.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static) {
                return true;
            }
        }
        DeclKind::Var => {
            if let Some(v) = ast.decls.var(first) {
                if v.spec.storage() == VarStorage::Static {
                    return true;
                }
                let q = ast.types.canonical(n.ty().unwrap_or(QualType::INT));
                let is_const = ast.types.effective_quals(q).is_const() || const_array(ast, q);
                if is_const
                    && !v.spec.is_inline()
                    && v.spec.storage() != VarStorage::Extern
                    && !is_extern_c(ast, first)
                {
                    return true;
                }
            }
        }
        _ => {}
    }
    let mut c = n.ctx();
    while let Some(ctx) = c {
        if ast.decls.get(ctx).kind() == DeclKind::Namespace && is_anon_ns(ast, ctx) {
            return true;
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
    match ast.names.view(n.name()) {
        DeclNameView::Identifier(s) => ast.symbols().as_str(s).to_owned(),
        _ => {
            if let Some(td) = ast.decls.typedef_for_anon(d) {
                return decl_name(ast, td);
            }
            ast.names.simple_spelling(n.name(), ast.symbols()).unwrap_or_default()
        }
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

/// The function whose body declares `d`, if any.
pub fn enclosing_function(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let mut c = ast.decls.get(d).ctx();
    while let Some(ctx) = c {
        if matches!(ast.decls.get(ctx).kind(), DeclKind::Function | DeclKind::CXXMethod) {
            return Some(ctx);
        }
        c = ast.decls.get(ctx).ctx();
    }
    None
}

/// The Itanium mangler state for one symbol.
struct Mangler<'a> {
    ast: &'a Ast,
    out: String,
    subs: Vec<Sub>,
}

/// The linkage name of a namespace-scope function or variable.
pub fn mangle_decl(ast: &Ast, d: DeclId) -> String {
    if is_unmangled(ast, d) {
        return decl_name(ast, d);
    }
    let mut m = Mangler { ast, out: String::from("_Z"), subs: Vec::new() };
    if let Some(f) = enclosing_function(ast, d) {
        // A static local variable: Z <function encoding> E <name> [_<n>].
        m.out.push('Z');
        m.encoding(f);
        m.out.push('E');
        m.source_name(&decl_name(ast, d));
        return m.out;
    }
    m.encoding(d);
    m.out
}

/// The linkage name of a static local `d`, the `index`-th (0-based)
/// static local of its function with the same name.
pub fn mangle_static_local(ast: &Ast, d: DeclId, index: u32) -> String {
    let mut s = mangle_decl(ast, d);
    if index > 0 {
        if index == 1 {
            s.push_str("_0");
        } else {
            s.push_str(&format!("__{}_", index - 1));
        }
    }
    s
}

impl Mangler<'_> {
    fn encoding(&mut self, d: DeclId) {
        let ast = self.ast;
        let n = ast.decls.get(d);
        let is_fn = matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod);
        if is_fn && (is_extern_c(ast, d) || (decl_name(ast, d) == "main" && in_global_ns(ast, d))) {
            // Unmangled functions appear in local names by source name only.
            self.source_name(&decl_name(ast, d));
            return;
        }
        self.name(d, has_internal_linkage(ast, d));
        if is_fn {
            let ty = n.ty().map(|t| ast.types.canonical(t));
            if let Some(p) = ty.and_then(|t| ast.types.proto(t)) {
                let params: Vec<QualType> = ast.types.params(p).to_vec();
                let variadic = p.variadic();
                if params.is_empty() && !variadic {
                    self.out.push('v');
                }
                for q in params {
                    self.ty(q.without_quals());
                }
                if variadic {
                    self.out.push('z');
                }
            }
        }
    }

    /// <name> for an entity: unscoped or nested.
    fn name(&mut self, d: DeclId, internal: bool) {
        let prefixes = self.prefix_chain(d);
        let nm = decl_name(self.ast, d);
        if prefixes.is_empty() {
            if internal {
                self.out.push('L');
            }
            self.source_name(&nm);
            return;
        }
        if prefixes.len() == 1 && self.is_std(prefixes[0]) {
            self.out.push_str("St");
            if internal {
                self.out.push('L');
            }
            self.source_name(&nm);
            return;
        }
        self.out.push('N');
        self.prefix(&prefixes);
        if internal {
            self.out.push('L');
        }
        self.source_name(&nm);
        self.out.push('E');
    }

    /// The enclosing namespaces/classes of `d`, outermost first, skipping
    /// linkage specifications and inline namespaces' transparency (inline
    /// namespaces are still mangled).
    fn prefix_chain(&self, d: DeclId) -> Vec<DeclId> {
        let mut v = Vec::new();
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(ctx) = c {
            match self.ast.decls.get(ctx).kind() {
                DeclKind::Namespace | DeclKind::CXXRecord | DeclKind::Enum => v.push(ctx),
                DeclKind::TranslationUnit => break,
                _ => {}
            }
            c = self.ast.decls.get(ctx).ctx();
        }
        v.reverse();
        v
    }

    fn is_std(&self, ns: DeclId) -> bool {
        self.ast.decls.get(ns).kind() == DeclKind::Namespace
            && decl_name(self.ast, ns) == "std"
            && self.prefix_chain(ns).is_empty()
    }

    /// Emits a nested-name prefix, using and recording substitutions.
    fn prefix(&mut self, chain: &[DeclId]) {
        // Find the longest already-substituted prefix.
        let mut start = 0;
        for i in (0..chain.len()).rev() {
            if let Some(k) = self.find(Sub::Decl(self.ast.first(chain[i]))) {
                self.sub_ref(k);
                start = i + 1;
                break;
            }
        }
        for (i, &p) in chain.iter().enumerate().skip(start) {
            if i == 0 && self.is_std(p) {
                self.out.push_str("St");
                continue;
            }
            let nm = self.component_name(p);
            self.source_name(&nm);
            self.subs.push(Sub::Decl(self.ast.first(p)));
        }
    }

    fn component_name(&self, d: DeclId) -> String {
        if self.ast.decls.get(d).kind() == DeclKind::Namespace && is_anon_ns(self.ast, d) {
            return "_GLOBAL__N_1".to_owned();
        }
        decl_name(self.ast, d)
    }

    fn source_name(&mut self, s: &str) {
        self.out.push_str(&s.len().to_string());
        self.out.push_str(s);
    }

    fn find(&self, s: Sub) -> Option<usize> {
        self.subs.iter().position(|&x| x == s)
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

    /// A tag type: its (possibly nested) name.
    fn tag(&mut self, d: DeclId) {
        let key = Sub::Decl(self.ast.first(d));
        if let Some(k) = self.find(key) {
            self.sub_ref(k);
            return;
        }
        let chain = self.prefix_chain(d);
        if chain.is_empty() {
            let nm = self.component_name(d);
            self.source_name(&nm);
        } else if chain.len() == 1 && self.is_std(chain[0]) {
            self.out.push_str("St");
            let nm = self.component_name(d);
            self.source_name(&nm);
        } else {
            self.out.push('N');
            self.prefix(&chain);
            let nm = self.component_name(d);
            self.source_name(&nm);
            self.out.push('E');
        }
        self.subs.push(key);
    }

    fn ty(&mut self, q: QualType) {
        let ast = self.ast;
        let q = ast.types.canonical(q);
        let quals = ast.types.effective_quals(q);
        let unq = q.without_quals();
        if !quals.is_empty() {
            let key = Sub::Type(q);
            if let Some(k) = self.find(key) {
                self.sub_ref(k);
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
            self.ty(unq);
            self.subs.push(key);
            return;
        }
        match ast.types.canonical_view(unq) {
            TypeView::Builtin(b) => self.out.push_str(builtin_code(b)),
            TypeView::Record(d) | TypeView::Enum(d) => self.tag(d),
            view => {
                let key = Sub::Type(unq);
                if let Some(k) = self.find(key) {
                    self.sub_ref(k);
                    return;
                }
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
                        let n = ast.types.array_size(unq).unwrap_or(0);
                        self.out.push('A');
                        self.out.push_str(&n.to_string());
                        self.out.push('_');
                        if let Some(e) = ast.types.element(unq) {
                            self.ty(e);
                        }
                    }
                    TypeView::IncompleteArray(e) => {
                        self.out.push_str("A_");
                        self.ty(e);
                    }
                    TypeView::Function(p) => {
                        self.out.push('F');
                        self.ty(p.ret());
                        let params: Vec<QualType> = ast.types.params(&p).to_vec();
                        if params.is_empty() && !p.variadic() {
                            self.out.push('v');
                        }
                        for x in params {
                            self.ty(x.without_quals());
                        }
                        if p.variadic() {
                            self.out.push('z');
                        }
                        self.out.push('E');
                    }
                    _ => self.out.push_str("u7unknown"),
                }
                self.subs.push(key);
            }
        }
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
