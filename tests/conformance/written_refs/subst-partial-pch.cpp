// Substitution keys through a precompiled header (spec/ast.md §18.10.1,
// §18.14.5): the prefix matches `can_swap<sp::S>`
// ([temp.spec.partial.match]/2) and its snapshot carries the key, its
// rows and its root. The main file names `can_swap<sp::S>` again (the
// same specialization, [temp.type]/1: no new match) and matches
// `can_swap<sp::V>` anew. The dump, keys and roots included, is the one
// subst-partial-pch-include.cpp gives by including the header as text.
//@ pch: Inputs/subst_partial_pch.h
//@ written-refs: subst-partial-pch.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
bool again = can_swap<sp::S>::value;
bool anew = can_swap<sp::V>::value;
