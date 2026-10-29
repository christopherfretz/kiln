//! The WP-066a class entry points (spec/sema-class.md §8.3): until their
//! work packages land, each reports `{what} is not supported yet` and
//! never panics; the calls every class definition makes (an empty
//! base-clause, no ctor-initializer, finishing the class contexts) are
//! silent so M2 programs are unaffected.
#![allow(clippy::many_single_char_names, reason = "s/r/f/h/p: sema, record, function, head, param")]

mod conv_common;

use conv_common::{L, fn_type, int_lit, name, with_sema};
use kiln_ast::DeclHead;
use kiln_ast::decl::{Access, DeclKind, FnSpec, TagKind};
use kiln_ast::types::ExcSpec;
use kiln_ast::{DeclId, QualType};
use kiln_diag::MemorySink;
use kiln_sema::{BaseSpecInfo, DeclSpec, Initializer, MemInitId, MemInitInfo, Sema};

fn messages(sink: &MemorySink) -> Vec<String> {
    sink.take().into_iter().map(|d| format!("{}: {}", d.level.text_name(), d.message)).collect()
}

fn unsupported(what: &str) -> Vec<String> {
    vec![format!("error: {what} is not supported yet")]
}

/// `struct S {` (or `class S {`): a class being defined, with a member
/// function `void f()` declared in it.
fn open_class(s: &mut Sema<'_, '_>, tag: TagKind) -> (DeclId, DeclId) {
    let tu = s.ast.tu;
    let h = DeclHead::new(tu, name(s, "S"), L);
    let r = s.ast.builder().record(h, tag, None);
    s.act_on_start_record_definition(r);
    let ft = fn_type(s, QualType::VOID, &[]);
    let h = DeclHead::new(r, name(s, "f"), L);
    let f = s.ast.builder().function(h, DeclKind::CXXMethod, ft, FnSpec::new(), None);
    (r, f)
}

#[test]
fn class_build_tracks_the_class_being_defined() {
    with_sema(|s, sink| {
        let (r, _) = open_class(s, TagKind::Class);
        let c = s.current_class().expect("pushed at `{`");
        assert_eq!((c.record, c.access), (r, Access::Private));
        s.act_on_finish_record_definition(r, true);
        assert!(s.current_class().is_none());
        let (r, _) = open_class(s, TagKind::Struct);
        assert_eq!(s.current_class().map(|c| c.access), Some(Access::Public));
        s.act_on_finish_record_definition(r, true);
        assert_eq!(messages(sink), Vec::<String>::new());
    });
}

#[test]
fn calls_every_class_makes_are_silent() {
    with_sema(|s, sink| {
        let (r, f) = open_class(s, TagKind::Struct);
        s.act_on_base_specifiers(r, &[]);
        s.act_on_mem_initializers(f, &[], L);
        s.act_on_finish_record_definition(r, true);
        s.act_on_finish_class_contexts(r);
        assert_eq!(messages(sink), Vec::<String>::new());
    });
}

#[test]
fn class_features_report_unsupported() {
    with_sema(|s, sink| {
        let (r, f) = open_class(s, TagKind::Struct);
        s.act_on_defaulted(f, L);
        assert_eq!(messages(sink), unsupported("defaulted functions"));
        s.act_on_deleted(f, L, None);
        assert_eq!(messages(sink), unsupported("deleted functions"));
        s.act_on_exception_spec(f, ExcSpec::BasicNoexcept);
        assert_eq!(messages(sink), unsupported("exception specifications of member functions"));
        let one = int_lit(s, QualType::INT, 1);
        let init = MemInitInfo {
            id: MemInitId::Ident(name(s, "x").as_ident().unwrap()),
            loc: L,
            init: Initializer::Paren(vec![one], L, L),
            pack_expansion: false,
        };
        s.act_on_mem_initializers(f, &[init], L);
        assert_eq!(messages(sink), unsupported("constructor initializers"));
        s.act_on_finish_record_definition(r, true);
    });
}

/// WP-066b: base-clauses, access specifiers, friends, default member
/// initializers, default arguments and `this` are implemented.
#[test]
fn class_features_landed_in_066b() {
    with_sema(|s, sink| {
        let (r, f) = open_class(s, TagKind::Struct);
        let base = BaseSpecInfo {
            ty: QualType::INT,
            loc: L,
            is_virtual: false,
            access: Access::None,
            pack_expansion: false,
            attrs: Vec::new(),
        };
        s.act_on_base_specifiers(r, &[base]);
        assert_eq!(messages(sink), vec!["error: base specifier must name a class".to_string()]);
        let a = s.act_on_access_spec(Access::Private, L).expect("AccessSpec decl");
        assert_eq!(s.ast.decls.get(a).kind(), DeclKind::AccessSpec);
        assert_eq!(s.current_class().map(|c| c.access), Some(Access::Private));
        assert!(s.act_on_friend(&DeclSpec::default(), QualType::INT, None, L).is_some());
        assert_eq!(messages(sink), Vec::<String>::new());
        let h = DeclHead::new(r, name(s, "x"), L);
        let x = s.ast.builder().field(h, QualType::INT, None, 0);
        s.act_on_start_field_init(x);
        let this = s.act_on_this(L);
        assert!(!s.is_error_expr(this));
        let one = int_lit(s, QualType::INT, 1);
        s.act_on_field_init(x, Initializer::Expr(one));
        assert_eq!(messages(sink), Vec::<String>::new());
        let pn = name(s, "p");
        let p = s.ast.builder().param(f, pn, L, QualType::INT, 0, 0);
        let two = int_lit(s, QualType::INT, 2);
        s.act_on_param_default_arg(p, two);
        assert_eq!(messages(sink), Vec::<String>::new());
        s.act_on_finish_record_definition(r, true);
        let this = s.act_on_this(L);
        assert!(s.is_error_expr(this));
        assert_eq!(
            messages(sink),
            vec!["error: invalid use of 'this' outside of a non-static member function".to_string()]
        );
    });
}
