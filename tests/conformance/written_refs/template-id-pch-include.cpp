// template-id-pch.cpp's prefix included as text (spec/ast.md §18.5): the
// same golden (the two files keep their code on the same lines).
//@ written-refs: template-id-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
#include "Inputs/tid_pch.h"
//
//
//
//
int run() { return tp::g(1) + tp::S<int>{}.a() + tp::v<int>; }
