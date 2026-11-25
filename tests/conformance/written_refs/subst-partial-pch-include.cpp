// Substitution keys through a precompiled header (spec/ast.md §18.14.5):
// subst-partial-pch.cpp's prefix included as text, without a precompiled
// header; subst-partial-pch.cpp's dump is this one (the two files keep
// their code on the same lines).
//@ written-refs: subst-partial-pch.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
#include "Inputs/subst_partial_pch.h"
//
//
//
bool again = can_swap<sp::S>::value;
bool anew = can_swap<sp::V>::value;
