// Stage 5 (spec/ast.md §18.10.1): pch.cpp's prefix included as text,
// without a precompiled header; pch.cpp's dump is this one (the two
// files keep their code on the same lines).
//@ written-refs: pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
#include "Inputs/refs_pch.h"
//
//
pch_ns::Box<int> boxed{1};
int after_pch(int v) {
  return v + pch_ns::from_pch + pch_ns::measure(pch_ns::Pt{v}) + outer::twice({v}) + boxed.get();
}
