// [temp.type]/2.4: specializations for `&u.a` and `&u.b` of one union
// are distinct entities with distinct mangled names, so both link and
// each is called (kiln gave them one symbol before WP-2200). Substituted
// into a template, each argument designates its own member: a constant
// evaluation reads the active member `b` through `&cu.b`, a reference
// argument and a class-type argument holding the pointer. Reported by a
// downstream user.
//@ expect: run
//@ std: c++26
//@ stdout: 1 2 3 4
//@ stdout: 10 10
//@ stdout: 30 30
//@ stdout: 5 5 5
#include <cstdio>

template <auto V> struct A {};
template <auto &R> struct Ref {};
union U { int a; int b; } u;
struct S1 { int x; };
struct S2 { int x; };
union V { S1 s; S2 t; } v;

int f(A<&u.a>) { return 1; }
int f(A<&u.b>) { return 2; }
int g(A<&v.s.x>) { return 3; }
int g(A<&v.t.x>) { return 4; }
template <auto P> int tag() { return P == &u.a ? 10 : 20; }
template <auto &R> int rtag() { return &R == &u.b ? 30 : 40; }

constexpr union U2 { int a; int b; } cu{.b = 5};
template <const int *P> constexpr int read() { return *P; }
template <const int &R> constexpr int read_ref() { return R; }
struct P { const int *p; };
template <P X> constexpr int read_class() { return *X.p; }
static_assert(read<&cu.b>() == 5);
static_assert(read_ref<cu.b>() == 5);
static_assert(read_class<P{&cu.b}>() == 5);

int main() {
  std::printf("%d %d %d %d\n", f(A<&u.a>{}), f(A<&u.b>{}), g(A<&v.s.x>{}), g(A<&v.t.x>{}));
  std::printf("%d %d\n", tag<&u.a>(), tag<&u.b>());
  std::printf("%d %d\n", rtag<u.a>(), rtag<u.b>());
  std::printf("%d %d %d\n", read<&cu.b>(), read_ref<cu.b>(), read_class<P{&cu.b}>());
}
