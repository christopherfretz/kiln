// A function template's default template argument (spec/ast.md §18.9.2,
// §18.10.1): the row the prefix's deduction recorded for
// `call<dp::S, 4>` is carried in the snapshot, and the main file's
// deduction records `call<dp::U, 8>`'s; the dump is the one a compile
// that includes the header gives (inst-default-targ-pch-include.cpp).
//@ pch: Inputs/default_targ_pch.h
//@ written-refs: inst-default-targ-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
int run() { return dp::call<dp::U>() + dp::in_prefix(); }
