// Stage 5 (spec/ast.md §18.10.1): a precompiled header made with the
// table on carries both tables in its snapshot. The consumer's table
// continues the prefix's (its entries first, the main file's after
// them) and its instantiation of the prefix's template records its
// rows: the dump is the one a compile that includes the header gives.
//@ pch: Inputs/refs_pch.h
//@ written-refs: pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
pch_ns::Box<int> boxed{1};
int after_pch(int v) {
  return v + pch_ns::from_pch + pch_ns::measure(pch_ns::Pt{v}) + outer::twice({v}) + boxed.get();
}
int doubled_pt(pch_ns::Pt p) { return pch_ns::doubled(p).x + pch_ns::summed(p, p, p).x; }
