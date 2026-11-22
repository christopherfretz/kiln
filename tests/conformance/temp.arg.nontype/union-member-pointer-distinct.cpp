// [temp.type]/2.4: two pointer or reference template arguments are the
// same only when they designate the same object, so `&u.a` and `&u.b` of
// one union are different arguments even though they hold the same
// address and type: through a nested member of struct type, an array
// member's element, an anonymous union, a `void *`, a reference
// parameter, a pointer formed in a constant evaluation (through a call,
// a `void *` round trip, pointer arithmetic) and a class-type argument
// holding one. [temp.type]/2.7: a class-type argument of union type
// compares its active member too. Reported by a downstream user: kiln
// keyed both as `&u.a` (rejects-valid, one mangled name for two
// entities; WP-2200).
//@ expect: compile-ok
//@ std: c++26
#include <type_traits>

template <auto V> struct A {};
template <auto &R> struct Ref {};

union U { int a; int b; } u;
static_assert(!__is_same(A<&u.a>, A<&u.b>));
static_assert(!std::is_same_v<A<&u.a>, A<&u.b>>);
static_assert(std::is_same_v<A<&u.b>, A<&u.b>>);
static_assert(!std::is_same_v<Ref<u.a>, Ref<u.b>>);
static_assert(std::is_same_v<Ref<u.b>, Ref<u.b>>);

// Members of different types, converted to `void *`.
union X { int a; float f; } x;
static_assert(!std::is_same_v<A<(void *)&x.a>, A<(void *)&x.f>>);
static_assert(!std::is_same_v<A<(void *)&u.a>, A<(void *)&u.b>>);
static_assert(std::is_same_v<A<(void *)&u.b>, A<(void *)&u.b>>);

// A nested path: `x` at the same offset in two members of struct type.
struct S1 { int x; };
struct S2 { int x; };
union V { S1 s; S2 t; } v;
static_assert(!std::is_same_v<A<&v.s.x>, A<&v.t.x>>);
static_assert(!std::is_same_v<A<&v.s>, A<&v.t>>);

// Union members nested in union members.
union N { U p; U q; } n;
static_assert(!std::is_same_v<A<&n.p.a>, A<&n.p.b>>);
static_assert(!std::is_same_v<A<&n.p.a>, A<&n.q.a>>);
static_assert(!std::is_same_v<A<&n.p.b>, A<&n.q.b>>);
static_assert(!std::is_same_v<A<&n.p.b>, A<&n.q.a>>);

// An array member's element.
union W { int arr[2]; int b; } w;
static_assert(!std::is_same_v<A<&w.arr[0]>, A<&w.b>>);
static_assert(!std::is_same_v<A<&w.arr[1]>, A<&w.b>>);
static_assert(!std::is_same_v<A<w.arr>, A<&w.b>>);

// Through an anonymous union member.
struct O { int k; union { int p; int q; }; } o;
static_assert(!std::is_same_v<A<&o.p>, A<&o.q>>);

// A union member of a struct inside an array.
struct H { union { int i; int j; } m; };
H hs[2];
static_assert(!std::is_same_v<A<&hs[1].m.i>, A<&hs[1].m.j>>);
static_assert(!std::is_same_v<A<&hs[0].m.j>, A<&hs[1].m.j>>);

// A pointer formed in a constant evaluation.
constexpr int *pb = &u.b;
constexpr int *pa = &u.a;
static_assert(std::is_same_v<A<pb>, A<&u.b>>);
static_assert(std::is_same_v<A<pa>, A<&u.a>>);
static_assert(!std::is_same_v<A<pa>, A<pb>>);
constexpr int *id(int *p) { return p; }
static_assert(std::is_same_v<A<id(&u.b)>, A<&u.b>>);
static_assert(std::is_same_v<A<id(pb)>, A<&u.b>>);
constexpr int *via_void(int *p) { return static_cast<int *>(static_cast<void *>(p)); }
static_assert(std::is_same_v<A<via_void(&u.b)>, A<&u.b>>);
static_assert(std::is_same_v<A<&w.arr[0] + 1>, A<&w.arr[1]>>);
constexpr int &rb = u.b;
static_assert(std::is_same_v<Ref<rb>, Ref<u.b>>);
static_assert(std::is_same_v<A<&rb>, A<&u.b>>);
constexpr S2 *pt = &v.t;
static_assert(std::is_same_v<A<&pt->x>, A<&v.t.x>>);

// A class-type template argument holding such a pointer.
struct P { int *p; };
static_assert(!std::is_same_v<A<P{&u.a}>, A<P{&u.b}>>);
static_assert(std::is_same_v<A<P{pb}>, A<P{&u.b}>>);
constexpr P cp{&u.b};
static_assert(std::is_same_v<A<cp>, A<P{&u.b}>>);
static_assert(std::is_same_v<A<cp.p>, A<&u.b>>);

// Partial and explicit specialization.
template <class T> struct Which { static constexpr int value = 0; };
template <> struct Which<A<&u.a>> { static constexpr int value = 1; };
template <> struct Which<A<&u.b>> { static constexpr int value = 2; };
static_assert(Which<A<&u.a>>::value == 1);
static_assert(Which<A<&u.b>>::value == 2);
static_assert(Which<A<pb>>::value == 2);
static_assert(Which<A<&v.t.x>>::value == 0);
template <int *P, int *Q> struct Pair {};
template <class T> struct Diag { static constexpr bool value = false; };
template <int *P> struct Diag<Pair<P, P>> { static constexpr bool value = true; };
static_assert(!Diag<Pair<&u.a, &u.b>>::value);
static_assert(Diag<Pair<&u.b, pb>>::value);
static_assert(!Diag<Pair<&n.p.b, &n.q.b>>::value);

// Overloading.
constexpr int f(A<&u.a>) { return 1; }
constexpr int f(A<&u.b>) { return 2; }
static_assert(f(A<&u.a>{}) == 1);
static_assert(f(A<&u.b>{}) == 2);
static_assert(f(A<pb>{}) == 2);

// [temp.type]/2.7: union values with different active members.
static_assert(!std::is_same_v<A<U{.a = 1}>, A<U{.b = 1}>>);
static_assert(std::is_same_v<A<U{.b = 1}>, A<U{.b = 1}>>);
static_assert(!std::is_same_v<A<U{}>, A<U{.b = 0}>>);
static_assert(std::is_same_v<A<U{}>, A<U{.a = 0}>>);
struct E {};
union UE { E e; E g; };
static_assert(!std::is_same_v<A<UE{.e = {}}>, A<UE{.g = {}}>>);
