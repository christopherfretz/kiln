// The entries of a precompiled prefix's dependent template-ids
// (spec/ast.md §18.5, §18.10.1) are carried in its snapshot: a variable
// template-id names its template, a function template-id keeps its
// found set, a current-instantiation member template names the member.
// The dump is the one a compile that includes the header gives
// (template-id-pch-include.cpp).
//@ pch: Inputs/tid_pch.h
//@ written-refs: template-id-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
int run() { return tp::g(1) + tp::S<int>{}.a() + tp::v<int>; }
