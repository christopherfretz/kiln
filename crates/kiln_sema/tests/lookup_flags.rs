//! `LookupFlags::ADL_ONLY` (spec/sema-overload.md §2.3, spec/sema-class.md
//! §8.1): a friend first declared by its friend declaration is invisible
//! to ordinary unqualified and qualified lookup ([namespace.memdef]/3,
//! [basic.lookup.unqual]) until it is redeclared at namespace scope, but
//! redeclaration lookup sees it.

mod conv_common;

use conv_common::{L, fn_type, name, with_sema};
use kiln_ast::decl::{DeclKind, DeclMisc, FnSpec, FriendKind, TagKind};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_sema::lookup::{LookupFlags, LookupKind};

/// `struct C { friend void f(); };` then `void f();`.
#[test]
fn hidden_friend_is_adl_only_namespace_memdef_3() {
    with_sema(|s, _| {
        let tu = s.ast.tu;
        let ft = fn_type(s, QualType::VOID, &[]);
        let h = DeclHead::new(tu, name(s, "C"), L);
        let c = s.ast.builder().record(h, TagKind::Struct, None);
        s.ast.builder().begin_record_definition(c).unwrap();
        let fname = name(s, "f");
        let sym = fname.as_ident().unwrap();
        let h = DeclHead::new(tu, fname, L)
            .lexical(c)
            .misc(DeclMisc::new().with_friend(FriendKind::Undeclared));
        let f = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), None);
        let entries: Vec<_> = s.ast.lookup_local(tu, fname).iter_with_flags().collect();
        assert_eq!(entries, vec![(f, LookupFlags::ADL_ONLY)]);
        assert_eq!(s.lookup_name(sym, LookupKind::Ordinary).decls, Vec::<DeclId>::new());
        assert_eq!(s.lookup_qualified(tu, sym, LookupKind::Ordinary).decls, Vec::<DeclId>::new());
        assert_eq!(s.lookup_current_scope(sym), vec![f]);
        // `void f();` at namespace scope makes it visible.
        let h = DeclHead::new(tu, fname, L);
        let f2 = s.ast.builder().function(h, DeclKind::Function, ft, FnSpec::new(), Some(f));
        assert_eq!(s.lookup_name(sym, LookupKind::Ordinary).decls, vec![f2]);
        assert_eq!(s.lookup_qualified(tu, sym, LookupKind::Ordinary).decls, vec![f2]);
    });
}
