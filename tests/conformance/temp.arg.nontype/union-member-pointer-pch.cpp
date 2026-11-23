// [temp.type]/2.4 through a precompiled header: constant pointers to two
// members of one union, formed in the header, stay distinct in its user
// (the union path in the AST image and the Oven snapshot; WP-2200).
//@ expect: compile-ok
//@ std: c++26
//@ pch: Inputs/union_member_pointer_pch.h
static_assert(!same<A<&u.a>, A<&u.b>>);
static_assert(same<A<pb>, A<&u.b>>);
static_assert(same<AB, A<&u.b>>);
static_assert(same<decltype(ab), A<pb>>);
static_assert(same<A<&pt->x>, A<&v.t.x>>);
static_assert(!same<A<&pt->x>, A<&v.s.x>>);
static_assert(same<A<cp>, A<P{&u.b}>>);
static_assert(!same<A<cp>, A<P{&u.a}>>);
static_assert(*qb == 5);
constexpr const int *again = qb;
static_assert(same<A<again>, A<&cu.b>>);
