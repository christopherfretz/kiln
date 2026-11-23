// [temp.type]/2.4, Itanium C++ ABI 5.1.5.4 (`so <type> <expr> [<offset>]
// <union-selector>* [p] E`): a pointer or reference template argument
// designating a union member mangles its path's union selectors (`_` for
// a union's first member, `_<n-1>` for its n-th), so `&u.a` and `&u.b`
// name two functions, as clang 22 mangles them (one symbol before
// WP-2200; GCC 16.1 mangles the member access, `addtL_Z1uE1bE`, instead).
// Reported by a downstream user.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++26
template <auto V> int tag() { return 0; }
template <auto &R> int rtag() { return 0; }
union U { int a; int b; } u;
struct S1 { int x; };
struct S2 { int x; };
union V { S1 s; S2 t; } v;
union W { int arr[2]; int b; } w;
union X { int a; float f; } x;
struct O { int k; union { int p; int q; }; } o;
union N { U p; U q; } n;
struct P { int *p; };
template int tag<&u.a>();
template int tag<&u.b>();
template int tag<&v.s.x>();
template int tag<&v.t.x>();
template int tag<&v.s>();
template int tag<&v.t>();
template int tag<&w.arr[0]>();
template int tag<&w.arr[1]>();
template int tag<&w.b>();
template int tag<(void *)&x.f>();
template int tag<(void *)&u.b>();
template int tag<&o.p>();
template int tag<&o.q>();
template int tag<&n.p.a>();
template int tag<&n.p.b>();
template int tag<&n.q.a>();
template int tag<&n.q.b>();
template int tag<P{&u.a}>();
template int tag<P{&u.b}>();
template int rtag<u.a>();
template int rtag<u.b>();
template int rtag<n.q.b>();
