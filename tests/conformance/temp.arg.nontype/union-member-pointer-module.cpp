// [temp.type]/2.4 across a module: constant pointers to two members of
// one union, formed in the module, stay distinct in its importer (the
// union path of a carried pointer constant; WP-2200).
//@ module: union_member_pointer_mod=Inputs/union_member_pointer_mod.cppm
//@ two-step
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
