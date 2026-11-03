//! Filter mode: a dependent default template argument with the template's
//! own parameters substituted, spelled as clang prints the canonical type
//! (spec/ast-dump.md §6 "Selection"), and whether clang's structural
//! `isSubstitutedDefaultArgument` would call a written argument equal to it.
//!
//! The JSON type tree of the default is turned into a [`Term`] (a type
//! parameter becomes its argument's own tree), which prints the way
//! clang's `TypePrinter` nests declarators: `long (*)[4]`, `void (long)`,
//! `void (*)(long *, long *)`, `const long[3]`.

use super::print::{desugared, exact_int, flag, kids, kind, text};
use crate::util::json::Json;

/// A canonical type, as far as filter mode spells one.
enum Term {
    /// A type with no declarator part (`long`, `W<int>`), with its
    /// cv-qualifiers in clang's order.
    Atom {
        cv: Cv,
        name: String,
    },
    Pointer(Box<Term>, Cv),
    LRef(Box<Term>),
    RRef(Box<Term>),
    /// `elem[size]` (`elem[]` without a size).
    Array(Box<Term>, Option<i64>),
    Function {
        ret: Box<Term>,
        params: Vec<Term>,
        variadic: bool,
    },
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Cv {
    c: bool,
    v: bool,
}

impl Cv {
    fn parse(q: &str) -> Option<Cv> {
        let mut cv = Cv::default();
        for w in q.split(' ') {
            match w {
                "const" => cv.c = true,
                "volatile" => cv.v = true,
                _ => return None,
            }
        }
        Some(cv)
    }

    fn or(self, o: Cv) -> Cv {
        Cv { c: self.c || o.c, v: self.v || o.v }
    }

    fn spelled(self) -> &'static str {
        match (self.c, self.v) {
            (false, false) => "",
            (true, false) => "const",
            (false, true) => "volatile",
            (true, true) => "const volatile",
        }
    }
}

/// How a default compares with the argument it might have produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Defaulted {
    /// clang drops the argument whether or not it was written.
    Yes,
    /// clang prints the argument.
    No,
    /// The substituted default equals the argument, but clang's structural
    /// comparison of a written argument would not (an array or function
    /// declarator in the default, a reference collapsed through a type
    /// parameter, a qualified reference): clang drops the argument only if
    /// it was not written, which its JSON does not say.
    Unknown,
}

/// Builds terms for the default of the parameter at index `i` (depth
/// `depth`) from the template's arguments `args`; `exact` turns false as
/// soon as the substitution does something clang's structural comparison
/// does not follow.
struct Builder<'a, 'j> {
    args: &'a [&'j Json],
    i: usize,
    depth: Option<i64>,
    exact: bool,
}

impl Builder<'_, '_> {
    /// The term of type node `t` of the default (`pattern`: under the
    /// default's tree rather than an argument's).
    fn term(&mut self, t: &Json, pattern: bool) -> Option<Term> {
        let kid = |n: usize| kids(t).nth(n);
        match kind(t) {
            "TemplateTypeParmType" if pattern => {
                if t.get("depth").and_then(exact_int) != self.depth {
                    return None;
                }
                let ix = usize::try_from(t.get("index").and_then(exact_int)?)
                    .ok()
                    .filter(|&x| x < self.i)?;
                let arg = self.args[ix];
                match kids(arg).next() {
                    Some(at) => self.term(at, false),
                    None => atom(desugared(arg.get("type")?)?),
                }
            }
            "PointerType" => {
                let p = self.term(kid(0)?, pattern)?;
                if matches!(p, Term::LRef(_) | Term::RRef(_)) {
                    return None;
                }
                Some(Term::Pointer(Box::new(p), Cv::default()))
            }
            "LValueReferenceType" | "RValueReferenceType" => {
                let r = self.term(kid(0)?, pattern)?;
                let rvalue = kind(t) == "RValueReferenceType";
                // [dcl.ref]/7: a reference to a reference collapses; only
                // `&&` to `&&` stays an rvalue reference. clang's
                // comparison of a written argument does not collapse.
                Some(match r {
                    Term::LRef(_) => {
                        self.exact = false;
                        r
                    }
                    Term::RRef(inner) => {
                        self.exact = false;
                        if rvalue { Term::RRef(inner) } else { Term::LRef(inner) }
                    }
                    _ if rvalue => Term::RRef(Box::new(r)),
                    _ => Term::LRef(Box::new(r)),
                })
            }
            "QualType" => {
                let cv = Cv::parse(text(t, "qualifiers")?)?;
                let inner = self.term(kid(0)?, pattern)?;
                Some(self.qualify(inner, cv, pattern))
            }
            "ParenType" => self.term(kid(0)?, pattern),
            "DecayedType" => self.term(kids(t).last()?, pattern),
            "ConstantArrayType" | "IncompleteArrayType" => {
                if pattern {
                    self.exact = false;
                }
                let size = if kind(t) == "ConstantArrayType" {
                    Some(t.get("size").and_then(exact_int)?)
                } else {
                    None
                };
                let e = self.term(kid(0)?, pattern)?;
                if matches!(e, Term::LRef(_) | Term::RRef(_) | Term::Function { .. }) {
                    return None;
                }
                Some(Term::Array(Box::new(e), size))
            }
            "FunctionProtoType" => {
                if pattern {
                    self.exact = false;
                }
                // Only a plain prototype: no exception specification,
                // method qualifiers or trailing return type.
                let plain = t.as_obj().is_some_and(|o| {
                    o.iter().all(|(k, v)| match k.as_str() {
                        "kind"
                        | "id"
                        | "type"
                        | "inner"
                        | "isDependent"
                        | "isInstantiationDependent"
                        | "variadic" => true,
                        "cc" => v.as_str() == Some("cdecl"),
                        _ => false,
                    })
                });
                if !plain {
                    return None;
                }
                let mut it = kids(t);
                let ret = self.term(it.next()?, pattern)?;
                if matches!(ret, Term::Array(..) | Term::Function { .. }) {
                    return None;
                }
                let mut params = Vec::new();
                for p in it {
                    params.push(param(self.term(p, pattern)?));
                }
                Some(Term::Function { ret: Box::new(ret), params, variadic: flag(t, "variadic") })
            }
            // A partial specialization's argument names its own canonical
            // parameters (`type-parameter-0-0 *`).
            "TemplateTypeParmType" => atom(desugared(t.get("type")?)?),
            // A dependent type of any other kind (a specialization `W<T>`,
            // `typename T::type`) is not substituted here.
            _ if flag(t, "isDependent") => None,
            // A non-dependent leaf: its canonical spelling, if it has no
            // declarator part to re-nest.
            _ => atom(desugared(t.get("type")?)?),
        }
    }

    /// `inner` with `cv` added: on an atom merged in clang's order, on a
    /// pointer after its `*`, on an array's element; ignored on a
    /// reference ([dcl.ref]/1) and a function ([dcl.fct]/6).
    fn qualify(&mut self, inner: Term, cv: Cv, pattern: bool) -> Term {
        match inner {
            Term::Atom { cv: c, name } => Term::Atom { cv: c.or(cv), name },
            Term::Pointer(p, c) => Term::Pointer(p, c.or(cv)),
            Term::Array(e, n) => Term::Array(Box::new(self.qualify(*e, cv, pattern)), n),
            r @ (Term::LRef(_) | Term::RRef(_) | Term::Function { .. }) => {
                if pattern && cv != Cv::default() {
                    // clang's `getQualifiedType` keeps the qualifiers on
                    // the reference it compares a written argument with.
                    self.exact = false;
                }
                r
            }
        }
    }
}

/// A parameter type as a function type adjusts it ([dcl.fct]/5): arrays
/// and functions decay to pointers, top-level cv-qualifiers go.
fn param(t: Term) -> Term {
    match t {
        Term::Array(e, _) => Term::Pointer(e, Cv::default()),
        f @ Term::Function { .. } => Term::Pointer(Box::new(f), Cv::default()),
        Term::Atom { name, .. } => Term::Atom { cv: Cv::default(), name },
        Term::Pointer(p, _) => Term::Pointer(p, Cv::default()),
        r => r,
    }
}

/// A canonical spelling with no declarator part, split into its leading
/// cv-qualifiers and the rest; `None` for anything with one.
fn atom(s: &str) -> Option<Term> {
    // Declarator punctuation outside a template argument list.
    let mut angle = 0u32;
    for c in s.chars() {
        match c {
            '<' => angle += 1,
            '>' => angle = angle.saturating_sub(1),
            '*' | '&' | '(' | '[' if angle == 0 => return None,
            _ => {}
        }
    }
    let mut cv = Cv::default();
    let mut rest = s;
    while let Some((w, r)) = rest.split_once(' ') {
        match w {
            "const" => cv.c = true,
            "volatile" => cv.v = true,
            _ => break,
        }
        rest = r;
    }
    Some(Term::Atom { cv, name: rest.to_owned() })
}

/// `t` around the declarator text `inner`, as clang's `TypePrinter`
/// prints it: a pointer or reference to an array or function is
/// parenthesized, `*` binds its cv-qualifiers (`*const`), and a specifier
/// or qualifier is separated from the rest of the declarator by a space
/// unless that starts with `[` (`char *const[3]`).
fn print(t: &Term, inner: &str) -> String {
    let wrap = |p: &Term, op: &str, cv: Cv| {
        let q = cv.spelled();
        let mut d = format!("{op}{q}");
        if !inner.is_empty() {
            if !q.is_empty() && !inner.starts_with('[') {
                d.push(' ');
            }
            d.push_str(inner);
        }
        if matches!(p, Term::Array(..) | Term::Function { .. }) {
            d = format!("({d})");
        }
        print(p, &d)
    };
    match t {
        Term::Atom { cv, name } => {
            let q = cv.spelled();
            let mut s = if q.is_empty() { name.clone() } else { format!("{q} {name}") };
            if !inner.is_empty() {
                if !inner.starts_with('[') {
                    s.push(' ');
                }
                s.push_str(inner);
            }
            s
        }
        Term::Pointer(p, cv) => wrap(p, "*", *cv),
        Term::LRef(p) => wrap(p, "&", Cv::default()),
        Term::RRef(p) => wrap(p, "&&", Cv::default()),
        Term::Array(e, n) => {
            let n = n.map(|n| n.to_string()).unwrap_or_default();
            print(e, &format!("{inner}[{n}]"))
        }
        Term::Function { ret, params, variadic } => {
            let mut ps: Vec<String> = params.iter().map(|p| print(p, "")).collect();
            if *variadic {
                ps.push("...".into());
            }
            print(ret, &format!("{inner}({})", ps.join(", ")))
        }
    }
}

/// Whether argument `args[i]`, of type `arg_ty` (desugared), is the
/// dependent default type `def` (the default `TemplateArgument`'s type
/// tree) of the parameter at depth `depth`. `None` when the default has a
/// shape this module does not substitute.
pub(super) fn compare(
    def: &Json,
    args: &[&Json],
    i: usize,
    depth: Option<i64>,
    arg_ty: Option<&str>,
) -> Option<Defaulted> {
    let mut b = Builder { args, i, depth, exact: true };
    let t = b.term(def, true)?;
    Some(if arg_ty != Some(print(&t, "").as_str()) {
        Defaulted::No
    } else if b.exact {
        Defaulted::Yes
    } else {
        Defaulted::Unknown
    })
}
