// union-member-pointer-module.cpp through the module's declaration
// image (`-fkiln-bmi-image`): a pointer constant's union path is in the
// image (WP-2200).
//@ module: union_member_pointer_mod=Inputs/union_member_pointer_mod.cppm
//@ two-step
//@ args: -fkiln-bmi-image
//@ skip(clang): kiln-only flags
//@ std: c++26
//@ expect: compile-ok
import union_member_pointer_mod;
static_assert(!same<A<&u.a>, A<&u.b>>);
static_assert(same<A<pb>, A<&u.b>>);
static_assert(same<AB, A<&u.b>>);
static_assert(same<A<&pt->x>, A<&v.t.x>>);
static_assert(!same<A<&pt->x>, A<&v.s.x>>);
static_assert(same<A<cp>, A<P{&u.b}>>);
static_assert(!same<A<cp>, A<P{&u.a}>>);
static_assert(*qb == 5);
