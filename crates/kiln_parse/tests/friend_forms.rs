//! A friend's written form (spec/ast.md §23, WP-3164): each friend with
//! written parts (a qualified declarator-id, a template-id, a special
//! member, a qualified class friend with an own head) has a `FriendForm`
//! row holding the qualifier and template arguments as written, its own
//! heads and requires-clause, and its entity ([class.friend],
//! [temp.friend]). Two sides differing only in an instantiation-dependent
//! expression ([temp.res.general]/6) give different rows, though their
//! targets are the same. Every source here compiles with clang++-19 and
//! clang++-22 (diagnostics as noted).

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::dump::AstNames;
use kiln_ast::friend_form::{FriendFormAnswer, FriendFormFlags, FriendFormKind};
use kiln_ast::ids::DeclId;
use kiln_ast::types::TypeNames;
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// What one parse gives: the row dump and the error messages.
struct Parsed {
    /// `Ast::dump_friend_forms`.
    dump: String,
    errors: Vec<String>,
}

/// Parses `src` (written references recorded when `refs`) and runs
/// `check` on the unit.
fn parse_with<R>(
    src: &str,
    refs: bool,
    check: impl FnOnce(&Ast, &SourceManager) -> R,
) -> (Parsed, R) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { written_refs: refs, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors =
        sink.take().iter().filter(|d| d.level >= Level::Error).map(|d| d.message.clone()).collect();
    let dump = tu.ast.dump_friend_forms(&sm, Some(file));
    let r = check(&tu.ast, &sm);
    (Parsed { dump, errors }, r)
}

fn parse(src: &str) -> Parsed {
    parse_with(src, false, |_, _| ()).0
}

fn clean(src: &str) -> String {
    let p = parse(src);
    assert!(p.errors.is_empty(), "{:?}\n{src}", p.errors);
    p.dump
}

fn name(ast: &Ast, d: DeclId) -> String {
    let mut s = String::new();
    AstNames::new(ast).write_decl_name(d, true, &mut s);
    s
}

/// The design's reproducers (1)-(5), with `E` the instantiation-dependent
/// argument.
fn reproducers(e: &str) -> String {
    format!(
        "template <int N> struct Bq {{ void g(); struct Y {{}}; template <int M> void t(); }};\n\
         int f(...);\n\
         template <int U> void h();\n\
         template <class T> struct S {{\n\
           friend void Bq<{e}>::g();\n\
           friend struct Bq<{e}>::Y;\n\
           friend void h<{e}>();\n\
           friend void Bq<8>::t<{e}>();\n\
           friend void Bq<sizeof(T)>::g();\n\
         }};\n\
         S<int> s;\n"
    )
}

#[test]
fn design_reproducers_golden() {
    let got = clean(&reproducers("sizeof(sizeof(f(T())))"));
    let want = "5:41 FriendForm function qualifier 'Bq<sizeof (sizeof (f(T())))>::' name 'g'@5:41\n\
                7:13 FriendForm specialization name 'h'@7:13 args <sizeof (sizeof (f(T())))> entity h@3:23 found [h@3:23]\n\
                8:20 FriendForm specialization qualifier 'Bq<8>::' name 't'@8:20 args <sizeof (sizeof (f(T())))> entity Bq<8>::t@1:75 found [Bq<8>::t@1:75]\n\
                9:28 FriendForm function qualifier 'Bq<sizeof(T)>::' name 'g'@9:28 entity Bq::g@1:35 dependent\n";
    assert_eq!(got, want);
}

/// (1)-(4): the two sides' targets are the same (`Bq<8>::g`, `Bq<8>::Y`,
/// the type `void ()`), their rows are not.
#[test]
fn instantiation_dependent_sides_differ() {
    let a = clean(&reproducers("sizeof(sizeof(f(T())))"));
    let b = clean(&reproducers("sizeof(long)"));
    let (la, lb): (Vec<_>, Vec<_>) = (a.lines().collect(), b.lines().collect());
    assert_eq!(la.len(), lb.len());
    // (1), (3), (4) differ in their row.
    for i in 0..3 {
        assert_ne!(la[i], lb[i], "row {i}");
    }
    // (5) is the same on both sides.
    assert_eq!(la[3], lb[3]);
    // (2) is a class friend: its type keeps the qualifier as written.
    let class_ty = |src: &str| {
        parse_with(src, false, |ast, _| {
            let names = AstNames::new(ast);
            ast.decls
                .ids()
                .filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend)
                .find_map(|d| match ast.decls.friend_target(d) {
                    FriendTarget::Type { ty, .. } if !ast.types.is_function(ty) => {
                        let o = kiln_ast::types::PrintOptions::default();
                        Some(kiln_ast::types::print_type(&ast.types, &names, ty, o))
                    }
                    _ => None,
                })
                .unwrap()
        })
        .1
    };
    assert_eq!(
        class_ty(&reproducers("sizeof(sizeof(f(T())))")),
        "struct Bq<sizeof (sizeof (f(T())))>::Y"
    );
    assert_eq!(class_ty(&reproducers("sizeof(long)")), "struct Bq<sizeof(long)>::Y");
}

const SPECIAL: &str = "template <class T> void h(T);\n\
                       template <class T> struct Z { Z(); ~Z(); operator int(); };\n\
                       struct P { void g(); };\n\
                       namespace N { void k(); }\n\
                       template <class T> struct A { void f(); struct In {}; };\n\
                       template <class T> struct S {\n\
                         friend void h<>(T);\n\
                         friend void h<int>(int);\n\
                         friend Z<T>::Z();\n\
                         friend Z<T>::~Z();\n\
                         friend Z<T>::operator int();\n\
                         friend void ::N::k();\n\
                         template <class U> friend void A<U>::f();\n\
                         template <class U> friend class A<U>::In;\n\
                         friend class A<T>::In;\n\
                         friend struct ::P;\n\
                       };\n\
                       struct Q { friend void N::k(); friend void P::g(); friend void h<long>(long); };\n";

/// Special members, [temp.friend]/5 friends (`-Wunsupported-friend`:
/// `UNSUPPORTED`), `::` and a non-templated class.
#[test]
fn special_members_and_every_spec_friends() {
    let got = clean(SPECIAL);
    let want = "7:13 FriendForm specialization name 'h'@7:13 args <> entity h@1:25 found [h@1:25] dependent\n\
                8:13 FriendForm specialization name 'h'@8:13 args <int> entity h@1:25 found [h@1:25]\n\
                9:14 FriendForm constructor qualifier 'Z<T>::' name 'Z<T>'@9:14 entity Z::Z<T>@2:31 dependent\n\
                10:14 FriendForm destructor qualifier 'Z<T>::' name '~Z<T>'@10:14 entity Z::~Z<T>@2:36 dependent\n\
                11:14 FriendForm conversion qualifier 'Z<T>::' name 'operator int'@11:14 entity Z::operator int@2:42 dependent\n\
                12:18 FriendForm function qualifier '::N::' name 'k'@12:18 global\n\
                13:38 FriendForm function qualifier 'A<U>::' name 'f'@13:38 heads 1 entity A::f@5:36 every-spec dependent unsupported\n\
                14:27 FriendForm class qualifier 'A<U>::' name 'In'@14:39 heads 1 every-spec dependent unsupported\n\
                18:27 FriendForm function qualifier 'N::' name 'k'@18:27\n\
                18:47 FriendForm function qualifier 'P::' name 'g'@18:47\n\
                18:64 FriendForm specialization name 'h'@18:64 args <long> found [h@1:25]\n";
    assert_eq!(got, want);
}

/// §23.4 through the API: kinds, flags, `Some(EMPTY)` for `h<>`, `None`
/// without a template-id, `entity()` the target when it is a `Decl`,
/// and no row for `friend class A<T>::In;` / `friend struct ::P;`.
#[test]
fn api_view() {
    let (p, ()) = parse_with(SPECIAL, false, |ast, _| {
        let mut kinds = Vec::new();
        for d in ast.decls.ids().filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend) {
            match ast.friend_form(d) {
                FriendFormAnswer::Row(v) => {
                    kinds.push(v.kind());
                    if v.kind() == FriendFormKind::Specialization
                        && v.flags().contains(FriendFormFlags::DEPENDENT)
                    {
                        assert_eq!(v.args().map(|a| ast.types.args(a).len()), Some(0));
                    }
                    if v.kind() == FriendFormKind::Function {
                        assert!(v.args().is_none());
                    }
                    if let FriendTarget::Decl(t) = ast.decls.friend_target(d) {
                        assert_eq!(v.entity(), Some(t));
                    }
                }
                FriendFormAnswer::None => {
                    // Only the class friends without an own head.
                    assert!(
                        matches!(ast.decls.friend_target(d), FriendTarget::Type { ty, .. } if !ast.types.is_function(ty))
                    );
                }
                FriendFormAnswer::NotCarried => panic!("not from an image"),
            }
        }
        assert!(kinds.contains(&FriendFormKind::Constructor));
        assert!(kinds.contains(&FriendFormKind::Class));
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
}

/// The instantiated friend `friend void h<>(T);` in `S<int>` befriends
/// `h<int>` itself and has no row (§23.1).
#[test]
fn instantiated_friend_targets_the_entity() {
    let src = format!("{SPECIAL}S<int> s;\n");
    let p = parse_with(&src, false, |ast, _| {
        let mut targets = Vec::new();
        for d in ast.decls.ids().filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend) {
            let ctx = ast.decls.get(d).ctx().unwrap();
            if ast.spec_info(ctx).is_none() {
                continue;
            }
            if let FriendTarget::Decl(t) = ast.decls.friend_target(d)
                && ast.decls.get(t).kind() == DeclKind::Function
                && ast.spec_info(t).is_some()
            {
                assert!(matches!(ast.friend_form(d), FriendFormAnswer::None), "{}", name(ast, t));
                let mut s = String::new();
                AstNames::new(ast).write_decl_name(t, true, &mut s);
                if let Some((_, i)) = ast.spec_info(t) {
                    let mut a = String::new();
                    kiln_ast::types::write_arg_list(
                        &ast.types,
                        &AstNames::new(ast),
                        i.args,
                        &mut a,
                    );
                    s.push_str(&a);
                }
                targets.push(s);
            }
        }
        targets
    });
    assert!(p.0.errors.is_empty(), "{:?}", p.0.errors);
    assert!(p.1.iter().any(|t| t == "h<int>"), "{:?}", p.1);
}

/// A friend template with a requires-clause: its head reaches the
/// parameters and the clause; two heads differing only in the
/// constraint give different rows.
#[test]
fn heads_with_constraints() {
    let src = |c: &str| {
        format!(
            "template <class U> concept C = true;\n\
             template <class U> concept D = true;\n\
             namespace N {{ template <class U> requires {c}<U> void f(U); }}\n\
             template <class T> struct S {{\n\
               template <class U> requires {c}<U> friend void N::f(U);\n\
             }};\n\
             S<int> s;\n"
        )
    };
    let head = |c: &str| {
        parse_with(&src(c), false, |ast, _| {
            let fd = ast.decls.friend_forms().next().expect("a row").0;
            let FriendFormAnswer::Row(v) = ast.friend_form(fd) else { panic!() };
            assert_eq!(v.heads().len(), 1);
            let h = v.head().expect("its own head");
            let params: Vec<DeclId> = v.head_params(h).collect();
            assert_eq!(params.len(), 1);
            assert!(name(ast, params[0]).ends_with('U'));
            let req = v.head_requires(h).expect("the head's requires-clause");
            let mut s = String::new();
            AstNames::new(ast).write_expr(req, &mut s);
            s
        })
    };
    let (pc, c) = head("C");
    let (pd, d) = head("D");
    assert!(pc.errors.is_empty() && pd.errors.is_empty(), "{:?} {:?}", pc.errors, pd.errors);
    assert!(c.starts_with("C<"), "{c}");
    assert!(d.starts_with("D<"), "{d}");
    assert_ne!(c, d);
}

/// A trailing requires-clause on a friend function template naming only
/// its own parameters ([temp.friend]/9 allows it in a declaration).
#[test]
fn trailing_requires() {
    let src = "template <class U> concept C = true;\n\
               namespace N { template <class U> void f(U) requires C<U>; }\n\
               template <class T> struct S {\n\
                 template <class U> friend void N::f(U) requires C<U>;\n\
               };\n";
    let (p, has) = parse_with(src, false, |ast, _| {
        let fd = ast.decls.friend_forms().next().expect("a row").0;
        let FriendFormAnswer::Row(v) = ast.friend_form(fd) else { panic!() };
        v.requires().is_some()
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert!(has);
    assert!(p.dump.contains(" requires"), "{}", p.dump);
}

/// `Bq<8>::template t<1>`: `TEMPLATE_KW`.
#[test]
fn template_keyword() {
    let got = clean(
        "template <int N> struct Bq { template <int M> void t(); };\n\
         struct S { friend void Bq<8>::template t<1>(); };\n",
    );
    assert_eq!(
        got,
        "2:40 FriendForm specialization qualifier 'Bq<8>::' name 't'@2:40 args <1> found [Bq<8>::t@1:52] template\n"
    );
}

/// §23.3: the written-reference entry at a row's `name_loc` has the row's
/// entity (the candidate of a `DEPENDENT` row, then `DEPENDENT` itself).
#[test]
fn written_refs_agree_with_rows() {
    let src = format!(
        "{}{SPECIAL}",
        reproducers("sizeof(sizeof(f(T())))")
            .replace("S<int> s;\n", "")
            .replace("struct S", "struct S0")
    );
    let (p, checked) = parse_with(&src, true, |ast, sm| {
        let refs = &ast.written_refs;
        let mut checked = 0;
        for (fd, r) in ast.decls.friend_forms() {
            let at = sm.spelling_loc(r.name_loc);
            let entry = refs.iter().find(|(_, e)| e.range.lo == at).map(|(_, e)| *e);
            let Some(e) = entry else { continue };
            let FriendFormAnswer::Row(v) = ast.friend_form(fd) else { panic!() };
            let want = match (r.entity, ast.decls.friend_target(fd)) {
                (Some(x), _) => Some(ast.first(x)),
                (None, FriendTarget::Decl(_)) => continue,
                (None, FriendTarget::Type { .. }) => None,
            };
            assert_eq!(e.entity, want, "{:?} at {:?}", r.kind, sm.line_col(at));
            let dep = e.flags & kiln_ast::written_refs::REF_DEPENDENT != 0;
            if v.flags().contains(FriendFormFlags::DEPENDENT) {
                assert!(dep, "the entry of a dependent row is dependent");
            }
            checked += 1;
        }
        checked
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert!(checked >= 6, "{checked}");
}
