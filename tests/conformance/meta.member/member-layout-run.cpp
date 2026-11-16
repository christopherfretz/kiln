// [meta.member]/2, /4 at run time: GCC's member-layout builtins with
// member pointers known only at run time. A pointer to member is
// pointer-interconvertible when it is the first member (offset 0) of a
// standard-layout class, or any member of a union; two correspond when
// they are equal and designate a member of the common initial sequence
// ([class.mem.general]/23). Found running GCC's C++ testsuite (WP-1472).
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not have the builtins
//@ expect: run
//@ stdout: 1 0 0 1 1 1 0 0 0
//@ std: c++20
#include <cstdio>

struct A { int a; int b; long c; };
struct B { int x; int y; char z; };
union U { int i; long l; };
struct N { int n; private: int m; };

[[gnu::noinline]] bool pi(int A::*m) { return __builtin_is_pointer_interconvertible_with_class(m); }
[[gnu::noinline]] bool pu(long U::*m) { return __builtin_is_pointer_interconvertible_with_class(m); }
[[gnu::noinline]] bool pn(int N::*m) { return __builtin_is_pointer_interconvertible_with_class(m); }
[[gnu::noinline]] bool cm(int A::*m1, int B::*m2) { return __builtin_is_corresponding_member(m1, m2); }

int main() {
  int A::*null = nullptr;
  std::printf("%d %d %d %d %d %d %d %d %d\n", pi(&A::a), pi(&A::b), pi(null), pu(&U::l),
              cm(&A::a, &B::x), cm(&A::b, &B::y), cm(&A::a, &B::y), cm(null, &B::x),
              pn(&N::n));
}
