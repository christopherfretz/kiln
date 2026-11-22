// Header of union-member-pointer-pch.cpp: pointers to union members
// (and a specialization keyed by one) that a precompiled header carries.
template <class, class> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
template <auto P> struct A {};
union U { int a; int b; };
inline U u;
struct S1 { int x; };
struct S2 { int x; };
union V { S1 s; S2 t; };
inline V v;
constexpr int *pb = &u.b;
constexpr S2 *pt = &v.t;
using AB = A<&u.b>;
inline AB ab;
struct P { int *p; };
constexpr P cp{&u.b};
constexpr U cu{.b = 5};
constexpr const int *qb = &cu.b;
