// The found sets of a precompiled prefix's templates (spec/ast.md
// §18.12, §18.10.1) are carried in its snapshot: the dump is the one a
// compile that includes the header gives (found-pch-include.cpp).
//
//
//@ pch: Inputs/found_pch.h
//@ written-refs: found-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
void run() {
  fp::call(1);
  fp::S<int>{}.use(1);
}
