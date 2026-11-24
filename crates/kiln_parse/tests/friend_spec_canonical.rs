//! A befriended class template specialization names the same type as
//! the same template-id written anywhere else ([temp.type]/1,
//! [temp.over.link]/5, WP-3270): `friend It<T, true>;`,
//! `friend class It<T, true>;`, a friend function's parameter of that
//! type and a friend naming a member template's specialization give the
//! canonical type a member's `It<T, true> *` points to, in the pattern
//! (dependent) and in each instantiation. Reduced from real code (a
//! tooling client identifies entities by canonical type). Every source
//! here compiles with clang++-19 and clang++-22.

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::ids::{DeclId, TypeId};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` (no errors allowed) and runs `check` on the unit.
fn parse<R>(src: &str, check: impl FnOnce(&Ast) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().iter().filter(|d| d.level >= Level::Error).map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}\n{src}");
    check(&tu.ast)
}

/// The classes named `name` (the pattern and every specialization), each
/// with whether it is the pattern.
fn classes(ast: &Ast, name: &str) -> Vec<(DeclId, bool)> {
    ast.decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::CXXRecord
                && ast.definition(d) == Some(d)
                && ast
                    .decls
                    .get(d)
                    .name()
                    .as_ident()
                    .is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .map(|d| (d, ast.spec_info(d).is_none()))
        .collect()
}

/// The canonical types class `c` names: each friend's (a class friend's
/// type, a friend function's first parameter's) and each data member's
/// pointee, in member order.
fn named_types(ast: &Ast, c: DeclId) -> (Vec<TypeId>, Vec<TypeId>) {
    let canon = |q| ast.types.canonical(q).ty();
    let (mut friends, mut fields) = (Vec::new(), Vec::new());
    for m in ast.decls.members(c) {
        match ast.decls.get(m).kind() {
            DeclKind::Friend => match ast.decls.friend_target(m) {
                FriendTarget::Type { ty, .. } => friends.push(canon(ty)),
                FriendTarget::Decl(f) => {
                    let ty = ast.decls.get(f).ty().expect("a function type");
                    let p = ast.types.proto(ty).expect("a prototype");
                    friends.push(canon(ast.types.params(p)[0]));
                }
            },
            DeclKind::Field => {
                let ty = ast.decls.get(m).ty().expect("a field type");
                fields.push(canon(ast.types.pointee(ty).expect("a pointer member")));
            }
            _ => {}
        }
    }
    (friends, fields)
}

/// Every friend of class `name` (pattern and specializations) names the
/// one type all its pointer members point to.
fn assert_one_type(src: &str, name: &str, specs: usize) {
    parse(src, |ast| {
        let cs = classes(ast, name);
        assert_eq!(cs.iter().filter(|c| c.1).count(), 1, "one pattern of {name}");
        assert_eq!(cs.iter().filter(|c| !c.1).count(), specs, "specializations of {name}");
        for (c, pattern) in cs {
            let (friends, fields) = named_types(ast, c);
            assert!(!friends.is_empty() && !fields.is_empty(), "{name} (pattern: {pattern})");
            for f in &friends {
                for m in &fields {
                    assert_eq!(f, m, "{name} (pattern: {pattern}): friend vs member\n{src}");
                }
            }
        }
    });
}

const IT: &str = "template <class T, bool B> struct It {};\n";

#[test]
fn friend_type_specifier() {
    let src = format!(
        "{IT}template <class T> struct C {{ friend It<T, true>; It<T, true> *p; }};\n\
         C<int> c1; C<long> c2;\n"
    );
    assert_one_type(&src, "C", 2);
}

#[test]
fn friend_elaborated_class() {
    let src = format!(
        "{IT}template <class T> struct C {{\n\
           friend class It<T, true>;\n\
           It<T, true> *p;\n\
         }};\n\
         C<int> c1;\n"
    );
    assert_one_type(&src, "C", 1);
}

#[test]
fn friend_function_parameter() {
    let src = format!(
        "{IT}template <class T> struct C {{\n\
           friend void f(It<T, true>) {{}}\n\
           It<T, true> *p;\n\
         }};\n\
         C<int> c1;\n"
    );
    assert_one_type(&src, "C", 1);
}

#[test]
fn friend_member_template_specialization() {
    let src = "template <class T> struct C {\n\
                 template <class U, int N> struct In {};\n\
                 friend class In<T, 2>;\n\
                 In<T, 2> *p;\n\
               };\n\
               C<int> c1;\n";
    assert_one_type(src, "C", 1);
}

#[test]
fn elaborated_member_type() {
    // A plain elaborated reference takes the same path as a friend's.
    let src = format!(
        "{IT}template <class T> struct C {{\n\
           friend It<T, true>;\n\
           struct It<T, true> *p;\n\
           class It<T, true> *q;\n\
         }};\n\
         C<int> c1;\n"
    );
    assert_one_type(&src, "C", 1);
}
