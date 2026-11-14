// [class.ctor.general]/1, [dcl.ambig.res]/1, [class.friend]/2 (found
// building an open-source serialization library): in a member-declaration
// the class name followed by `(` names a constructor only when a
// parameter-declaration-clause follows. `friend S (::g)(int);` is the
// class name as the decl-specifier and a parenthesized qualified
// declarator-id, so `::g` is a friend and may call the private
// constructor; likewise `(N::g)`, `(::operator""_s)`, a class template's
// injected-class-name with and without arguments, a nested class, and
// `inline`/`constexpr` before the name.
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8
#include <cstddef>
extern "C" int printf(const char *, ...);
class S;
S g(int);
namespace N { S g(int); }
constexpr S operator""_s(const char *, std::size_t);
inline S h(int);
class S {
  S(int v) : v(v) {}
  friend S (::g)(int);
  friend S (N::g)(int);
  friend constexpr S (::operator""_s)(const char *str, std::size_t n);
  friend inline S (::h)(int);
public:
  int v;
};
S g(int x) { return S(x); }
S N::g(int x) { return S(x + 1); }
constexpr S operator""_s(const char *, std::size_t n) { return S(int(n)); }
inline S h(int x) { return S(x); }

template <class T> class C;
template <class T> C<T> mk(T);
template <class T> C<T> mk2(T);
template <class T> class C {
  C(T v) : v(v) {}
  friend C (::mk<T>)(T);
  friend C<T> (::mk2<T>)(T);
public:
  T v;
};
template <class T> C<T> mk(T x) { return C<T>(x); }
template <class T> C<T> mk2(T x) { return C<T>(x); }

struct O {
  class I;
};
O::I gi(int);
class O::I {
  I(int v) : v(v) {}
  friend O::I (::gi)(int);
public:
  int v;
};
O::I gi(int x) { return O::I(x); }

int main() {
  printf("%d %d %d %d %d %d %d %d\n", g(1).v, N::g(1).v, "abc"_s.v, h(4).v, mk(5).v,
         mk2(6).v, gi(7).v, mk2(8).v);
}
