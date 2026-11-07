// [dcl.fct.def.general]/8 (implementation-defined `__PRETTY_FUNCTION__`,
// clang's PredefinedExpr::ComputeName and NamedDecl::
// printNestedNameSpecifier): a member of a class template specialization
// prints the specialization's arguments as clang stores them, canonical
// with every name scoped (`C<N::X>::m`, `(anonymous namespace)::`, an
// alias or a typedef looked through, a defaulted argument left out, an
// NTTP expression folded), and a closure type argument as
// `(lambda at F:L:C)`, in the scope and in the ` [T = ...]` list alike.
// kiln printed the scope's arguments unscoped (`C<X>::m`) and a closure
// without its location. Matches clang 19 and 22 byte for byte (the
// directories of the file name are cut, the harness passes a path).
//@ expect: run
//@ std: c++20
//@ stdout: void C<N::X>::m() [T = N::X, N = 3]
//@ stdout: void C<N::X, 1>::m() [T = N::X, N = 1]
//@ stdout: void C<N::Y<N::X>>::m() [T = N::Y<N::X>, N = 3]
//@ stdout: void C<N::(anonymous namespace)::Z>::m() [T = N::(anonymous namespace)::Z, N = 3]
//@ stdout: void C<(lambda at predefined-class-scope-args.cpp:91:12)>::m() [T = (lambda at predefined-class-scope-args.cpp:91:12), N = 3]
//@ stdout: void C<Loc>::m() [T = Loc, N = 3]
//@ stdout: void C<N::X>::t(U) [T = N::X, N = 3, U = (lambda at predefined-class-scope-args.cpp:91:12)]
//@ stdout: void C<int>::t(U) [T = int, N = 3, U = N::Y<int>]
//@ stdout: static void C<long, 4>::s() [T = long, N = 4]
//@ stdout: void C<long, 2>::m() [T = long, N = 2]
//@ stdout: void C<long, 1>::In::k() [T = long, N = 1]
//@ stdout: void C<const long *>::m() [T = const long *, N = 3]
//@ stdout: void C<(anonymous namespace)::Anon>::m() [T = (anonymous namespace)::Anon, N = 3]
//@ stdout: void D<N::e1>::m() [V = N::e1]
//@ stdout: void D<3U>::m() [V = 3U]
//@ stdout: void D<'a'>::m() [V = 'a']
//@ stdout: void D<(short)2>::m() [V = (short)2]
//@ stdout: void P<N::X, N::X, int *>::m() [Ts = <N::X, N::X, int *>]
//@ stdout: void P<>::m() [Ts = <>]
//@ stdout: void (anonymous namespace)::AC<N::X>::m() [T = N::X]
//@ stdout: void (anonymous namespace)::AC<(lambda at predefined-class-scope-args.cpp:91:12)>::m() [T = (lambda at predefined-class-scope-args.cpp:91:12)]
//@ stdout: void C<void (*)(N::X) noexcept>::m() [T = void (*)(N::X) noexcept, N = 3]
//@ stdout: void C<int[2]>::m() [T = int[2], N = 3]
//@ stdout: void C<L>::m() [T = L, N = 3]
#include <cstdio>
#include <cstring>
static void show(const char* s) {
  char out[512];
  int n = 0;
  for (const char* p = s; *p && n < 500;) {
    if (std::strncmp(p, " at ", 4) == 0) {
      std::memcpy(out + n, " at ", 4);
      n += 4;
      p += 4;
      const char* colon = std::strchr(p, ':');
      for (const char* q = p; colon && q < colon; ++q)
        if (*q == '/') p = q + 1;
      continue;
    }
    out[n++] = *p++;
  }
  out[n] = 0;
  std::puts(out);
}
namespace N {
struct X {};
template <class T> struct Y {};
enum E { e1 = 1 };
namespace {
struct Z {};
}
}  // namespace N
template <class T, int N = 3> struct C {
  void m() { show(__PRETTY_FUNCTION__); }
  static void s() { show(__PRETTY_FUNCTION__); }
  template <class U> void t(U) { show(__PRETTY_FUNCTION__); }
  struct In {
    void k() { show(__PRETTY_FUNCTION__); }
  };
};
template <auto V> struct D {
  void m() { show(__PRETTY_FUNCTION__); }
};
template <class... Ts> struct P {
  void m() { show(__PRETTY_FUNCTION__); }
};
namespace {
struct Anon {};
template <class T> struct AC {
  void m() { show(__PRETTY_FUNCTION__); }
};
}  // namespace
using NX = N::X;
typedef long I;
template <class T> using Al = C<T, 1 + 1>;
struct L {};
int main() {
  auto l = [] {};
  struct Loc {};
  C<N::X>().m();
  C<NX, 1>().m();
  C<N::Y<NX>>().m();
  C<N::Z>().m();
  C<decltype(l)>().m();
  C<Loc>().m();
  C<N::X>().t(l);
  C<int>().t(N::Y<int>{});
  C<I, 2 * 2>::s();
  Al<I>().m();
  C<I, sizeof(char)>::In().k();
  C<const I*>().m();
  C<Anon>().m();
  D<N::e1>().m();
  D<3u>().m();
  D<'a'>().m();
  D<(short)2>().m();
  P<N::X, NX, int*>().m();
  P<>().m();
  AC<N::X>().m();
  AC<decltype(l)>().m();
  C<void (*)(N::X) noexcept(true)>().m();
  C<int[2]>().m();
  C<L>().m();
}
