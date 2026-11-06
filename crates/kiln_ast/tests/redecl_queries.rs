//! `Ast::first` / `latest` / `definition` (spec/ast.md §4.4) agree with
//! their pre-WP-655 bodies, kept here as references, on every
//! declaration of a corpus with redeclaration chains, definitions and
//! templates (the `first`/`definition` bits of `DeclMisc` are caches).

mod common;

use common::{INT, builtin, new_ast};
use kiln_ast::decl::{DeclKind, FnSpec, TagKind, VarSpec};
use kiln_ast::names::DeclName;
use kiln_ast::{Ast, DeclHead, DeclId};
use kiln_source::Loc;

const L: Loc = Loc::INVALID;

mod reference {
    use kiln_ast::{Ast, DeclId};

    fn template_redecl(ast: &Ast, d: DeclId, first: bool) -> DeclId {
        let Some(pattern) = ast.template_info(d).and_then(|t| t.pattern) else { return d };
        let Some(link) = ast.decls.redecl(pattern) else { return d };
        let group = ast.redecls.get(link.group);
        let target = if first { group.first } else { group.latest };
        ast.described_template(target).unwrap_or(d)
    }

    pub fn first(ast: &Ast, d: DeclId) -> DeclId {
        match ast.decls.redecl(d) {
            Some(r) => ast.redecls.get(r.group).first,
            None => template_redecl(ast, d, true),
        }
    }

    pub fn latest(ast: &Ast, d: DeclId) -> DeclId {
        match ast.decls.redecl(d) {
            Some(r) => ast.redecls.get(r.group).latest,
            None => template_redecl(ast, d, false),
        }
    }

    pub fn definition(ast: &Ast, d: DeclId) -> Option<DeclId> {
        match ast.decls.redecl(d) {
            Some(r) => ast.redecls.get(r.group).definition,
            None => Some(d),
        }
    }
}

fn head(ast: &Ast, ctx: DeclId, s: &str) -> DeclHead {
    DeclHead::new(ctx, DeclName::ident(ast.symbols().intern(s)), L)
}

fn check_all(ast: &Ast) -> usize {
    let mut n = 0;
    for d in ast.decls.ids() {
        let misc = ast.decls.get(d).misc();
        assert_eq!(ast.first(d), reference::first(ast, d), "first({d:?})");
        assert_eq!(ast.latest(d), reference::latest(ast, d), "latest({d:?})");
        assert_eq!(ast.definition(d), reference::definition(ast, d), "definition({d:?})");
        assert!(!misc.is_first() || reference::first(ast, d) == d, "first bit of {d:?}");
        assert!(
            !misc.is_definition() || reference::definition(ast, d) == Some(d),
            "definition bit of {d:?}"
        );
        n += 1;
    }
    n
}

/// Every redeclarable kind with a chain, definitions set before and
/// after later redeclarations, a namespace reopened, class templates
/// redeclared and defined, and non-redeclarable declarations.
#[test]
fn queries_agree_with_reference_bodies() {
    let mut ast = new_ast();
    let tu = ast.tu;
    let int = builtin(INT);

    // Functions: declared, defined, redeclared.
    let h = head(&ast, tu, "f");
    let f1 = ast.builder().function(h, DeclKind::Function, int, FnSpec::new(), None);
    let f2 = ast.builder().function(h, DeclKind::Function, int, FnSpec::new(), Some(f1));
    let body = ast.builder().compound(&[], L, L);
    ast.builder().set_body(f2, body).unwrap();
    let f3 = ast.builder().function(h, DeclKind::Function, int, FnSpec::new(), Some(f2));
    let h = head(&ast, tu, "g");
    let g = ast.builder().function(h, DeclKind::Function, int, FnSpec::new(), None);

    // Variables and typedefs.
    let h = head(&ast, tu, "v");
    let v1 = ast.builder().var(h, int, VarSpec::new(), None);
    let _v2 = ast.builder().var(h, int, VarSpec::new(), Some(v1));
    let h = head(&ast, tu, "T");
    let t1 = ast.builder().typedef(h, DeclKind::Typedef, int, None);
    let _t2 = ast.builder().typedef(h, DeclKind::Typedef, int, Some(t1));

    // Namespaces: opened, reopened, with a nested record chain.
    let h = head(&ast, tu, "N");
    let n1 = ast.builder().namespace(h, false, None);
    let n2 = ast.builder().namespace(h, false, Some(n1));
    let hs = head(&ast, n2, "S");
    let s1 = ast.builder().record(hs, TagKind::Struct, None);
    let s2 = ast.builder().record(hs, TagKind::Struct, Some(s1));
    ast.builder().begin_record_definition(s2).unwrap();
    let h = head(&ast, s2, "m");
    let _m = ast.builder().function(h, DeclKind::CXXMethod, int, FnSpec::new(), None);
    let _s3 = ast.builder().record(hs, TagKind::Struct, Some(s2));

    // Enumerations.
    let h = head(&ast, tu, "E");
    let e1 = ast.builder().enumeration(h, 0, Some((int, int)), None);
    let _e2 = ast.builder().enumeration(h, 0, Some((int, int)), Some(e1));

    // Class templates: declared, redeclared, defined through the pattern.
    let h = head(&ast, tu, "C");
    let ht = head(&ast, tu, "U");
    let mut b = ast.builder();
    let p1 = b.template_type_parm(ht, 0, 0, false, false);
    let pl1 = b.template_param_list(L, L, L, 0, &[p1], None);
    let r1 = b.record(h, TagKind::Struct, None);
    let c1 = b.template_decl(DeclKind::ClassTemplate, r1, pl1, None);
    let p2 = b.template_type_parm(ht, 0, 0, false, false);
    let pl2 = b.template_param_list(L, L, L, 0, &[p2], None);
    let r2 = b.record(h, TagKind::Struct, Some(r1));
    let c2 = b.template_decl(DeclKind::ClassTemplate, r2, pl2, Some(c1));
    b.begin_record_definition(r2).unwrap();
    let p3 = b.template_type_parm(ht, 0, 0, false, false);
    let pl3 = b.template_param_list(L, L, L, 0, &[p3], None);
    let r3 = b.record(h, TagKind::Struct, Some(r2));
    let c3 = b.template_decl(DeclKind::ClassTemplate, r3, pl3, Some(c2));

    // Function template.
    let h = head(&ast, tu, "ft");
    let mut b = ast.builder();
    let p4 = b.template_type_parm(ht, 0, 0, false, false);
    let pl4 = b.template_param_list(L, L, L, 0, &[p4], None);
    let fp = b.function(h, DeclKind::Function, int, FnSpec::new(), None);
    let ft = b.template_decl(DeclKind::FunctionTemplate, fp, pl4, None);

    // The cases the bits are for.
    for d in [f1, g, v1, t1, n1, s1, e1, c1, ft, r1, fp] {
        assert_eq!(ast.first(d), d);
    }
    for d in [c2, c3] {
        assert_eq!(ast.first(d), c1);
        assert_eq!(ast.latest(d), c3);
    }
    assert_eq!(ast.first(f3), f1);
    assert_eq!(ast.definition(f3), Some(f2));
    assert_eq!(ast.definition(s1), Some(s2));
    assert_eq!(ast.definition(r1), Some(r2));
    assert_eq!(ast.primary_context(n2), n1);
    assert!(ast.decls.get(f2).misc().is_definition());
    assert!(ast.decls.get(c1).misc().is_first() && !ast.decls.get(c2).misc().is_first());

    let n = check_all(&ast);
    assert!(n > 25, "corpus has {n} declarations");
}
