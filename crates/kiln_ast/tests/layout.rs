//! Node size assertions (spec/ast.md §14 `layout.rs`; sizes from §4.1,
//! §4.2, §5.1, §6, §7). A failing assert means a header grew: every
//! byte is multiplied by the node count of large constexpr TUs.

use std::mem::size_of;

use kiln_ast::attr::AttrNode;
use kiln_ast::decl::{DeclNode, FunctionData, RecordData, VarData};
use kiln_ast::expr::ExprNode;
use kiln_ast::names::DeclName;
use kiln_ast::redecl::RedeclGroup;
use kiln_ast::stmt::StmtNode;
use kiln_ast::{DeclId, ExprId, ListId, QualType, StmtId, TypeId};

#[test]
fn header_sizes_spec_ast_4_1_5_1_6_7() {
    assert_eq!(size_of::<DeclNode>(), 28);
    assert_eq!(size_of::<ExprNode>(), 20);
    assert_eq!(size_of::<StmtNode>(), 16);
    assert_eq!(size_of::<AttrNode>(), 16);
}

#[test]
fn payload_sizes_spec_ast_4_2_4_4() {
    assert_eq!(size_of::<FunctionData>(), 24);
    assert_eq!(size_of::<VarData>(), 24);
    assert_eq!(size_of::<RecordData>(), 40);
    assert_eq!(size_of::<RedeclGroup>(), 12);
}

#[test]
fn ids_are_niche_optimized_spec_ast_2_1() {
    assert_eq!(size_of::<Option<ExprId>>(), 4);
    assert_eq!(size_of::<Option<DeclId>>(), 4);
    assert_eq!(size_of::<Option<StmtId>>(), 4);
    assert_eq!(size_of::<Option<TypeId>>(), 4);
    assert_eq!(size_of::<Option<ListId>>(), 4);
    assert_eq!(size_of::<Option<QualType>>(), 4);
    assert_eq!(size_of::<DeclName>(), 4);
}
