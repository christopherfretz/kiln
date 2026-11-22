// Module of union-member-pointer-module.cpp: pointers to union members
// (and a specialization keyed by one) that a module exports.
export module union_member_pointer_mod;
export template <class, class> constexpr bool same = false;
export template <class T> constexpr bool same<T, T> = true;
export template <auto P> struct A {};
export union U { int a; int b; };
export U u;
export struct S1 { int x; };
export struct S2 { int x; };
export union V { S1 s; S2 t; };
export V v;
export constexpr int *pb = &u.b;
export constexpr S2 *pt = &v.t;
export using AB = A<&u.b>;
export struct P { int *p; };
export constexpr P cp{&u.b};
export constexpr U cu{.b = 5};
export constexpr const int *qb = &cu.b;
