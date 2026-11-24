// inst-default-targ-pch.cpp's prefix included as text (spec/ast.md
// §18.9.2): the same golden (the two files keep their code on the same
// lines).
//
//
//@ written-refs: inst-default-targ-pch.refs
//@ args: -fkiln-written-refs
//@ std: c++20
#include "Inputs/default_targ_pch.h"
int run() { return dp::call<dp::U>() + dp::in_prefix(); }
