//! `Sema::same_entity` (spec/sema.md §4 "Entity identity"; WP-471):
//! identity by redeclaration chain ([basic.link], [basic.def]/1).

mod conv_common;

use conv_common::{L, fn_type, name, with_sema};
use kiln_ast::decl::{DeclKind, FnSpec, FriendTarget, TagKind, VarSpec};
use kiln_ast::{DeclHead, QualType};

/// [basic.def]/1: a redeclaration and a definition after a declaration
/// name the entity the first declaration introduced; distinct
/// declarations of the same kind do not.
#[test]
fn redeclarations_are_one_entity_basic_def_1() {
    with_sema(|s, _| {
        let tu = s.ast.tu;
        let (sn, tn) = (name(s, "S"), name(s, "T"));
        let fwd = s.ast.builder().record(DeclHead::new(tu, sn, L), TagKind::Struct, None);
        let def = s.ast.builder().record(DeclHead::new(tu, sn, L), TagKind::Struct, Some(fwd));
        let other = s.ast.builder().record(DeclHead::new(tu, tn, L), TagKind::Struct, None);
        assert!(s.same_entity(fwd, def) && s.same_entity(def, fwd));
        assert!(!s.same_entity(def, other));
        assert_eq!(s.entity_decl(def), fwd);

        let nn = name(s, "N");
        let n1 = s.ast.builder().namespace(DeclHead::new(tu, nn, L), false, None);
        let n2 = s.ast.builder().namespace(DeclHead::new(tu, nn, L), false, Some(n1));
        let mn = name(s, "M");
        let m = s.ast.builder().namespace(DeclHead::new(tu, mn, L), false, None);
        assert!(s.same_entity(n2, n1) && !s.same_entity(n2, m));

        let vn = name(s, "v");
        let v1 = s.ast.builder().var(DeclHead::new(tu, vn, L), QualType::INT, VarSpec::new(), None);
        let v2 =
            s.ast.builder().var(DeclHead::new(tu, vn, L), QualType::INT, VarSpec::new(), Some(v1));
        assert!(s.same_entity(v1, v2) && !s.same_entity(v1, fwd));
    });
}

/// [class.mfct]/1, [class.friend]/1: an out-of-line member definition
/// is the member; a friend declaration of a function declares that
/// function (looked through), so it is the entity too.
#[test]
fn out_of_line_members_and_friends_are_the_entity_class_friend_1() {
    with_sema(|s, _| {
        let tu = s.ast.tu;
        let ft = fn_type(s, QualType::VOID, &[]);
        let cn = name(s, "C");
        let c = s.ast.builder().record(DeclHead::new(tu, cn, L), TagKind::Struct, None);
        s.ast.builder().begin_record_definition(c).unwrap();
        let mn = name(s, "m");
        let m = s.ast.builder().function(
            DeclHead::new(c, mn, L),
            DeclKind::CXXMethod,
            ft,
            FnSpec::new(),
            None,
        );
        let fname = name(s, "f");
        let f = s.ast.builder().function(
            DeclHead::new(tu, fname, L),
            DeclKind::Function,
            ft,
            FnSpec::new(),
            None,
        );
        let fd = s.ast.builder().function(
            DeclHead::new(tu, fname, L).lexical(c),
            DeclKind::Function,
            ft,
            FnSpec::new(),
            Some(f),
        );
        let friend = s.ast.builder().friend(c, L, FriendTarget::Decl(fd));
        s.ast.builder().complete_record_definition(c, kiln_ast::decl::DefinitionBits::empty());
        let out = s.ast.builder().function(
            DeclHead::new(c, mn, L).lexical(tu),
            DeclKind::CXXMethod,
            ft,
            FnSpec::new(),
            Some(m),
        );
        assert!(s.same_entity(out, m));
        assert!(s.same_entity(friend, f) && s.same_entity(fd, f) && s.same_entity(f, friend));
        assert_eq!(s.entity_decl(friend), f);
        assert!(!s.same_entity(friend, m) && !s.same_entity(out, f));
        // A friend declaration is not its class.
        assert!(!s.same_entity(friend, c));
    });
}
