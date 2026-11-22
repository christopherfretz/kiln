// Stage 5 (spec/ast.md §18.10.3): a precompiled header made with the
// table off carries no table; the consumer records its own names and
// lists the prefix as unrecorded, the dump's first line, so a client
// never mistakes the prefix for a file without names.
//@ pch: Inputs/refs_pch.h
//@ written-refs: pch-off.refs
//@ std: c++20
pch_ns::Box<int> boxed{1};
int after_pch(int v) { return v + pch_ns::measure(pch_ns::Pt{v}) + boxed.get(); }
