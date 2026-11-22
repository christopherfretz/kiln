// The declaration-extent table (spec/ast.md §19.6): the rows of a
// precompiled header's declarations survive the snapshot; the
// consumer's own declarations follow them.
//@ pch: Inputs/extents_pch.h
//@ decl-extents: pch.extents
//@ args: -fkiln-decl-extents
//@ std: c++20
pch_ns::Box<int> boxed{1};
int after_pch(int v) { return v + pch_ns::from_pch; }
