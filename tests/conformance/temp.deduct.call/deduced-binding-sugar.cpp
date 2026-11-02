// [temp.deduct.call]/1, [temp.deduct.type]/2, [over.match.viable]: the
// template-argument bindings in "candidate template ignored" notes print a
// deduced type as the call argument was written (`N::H<int>`, `H<int>` after a
// using-declaration), a canonical class type scoped (`N2::P`), conflicting
// deductions with their `aka`, and an unnamed template parameter as `$N`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 60:3 error "no matching function for call to 'f'"
//@ diag: 56:38 note "candidate template ignored: constraints not satisfied [with X = N::H<int>]"
//@ diag: 56:28 note "because 'N::H<int>' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::H<int>) > 100' (1 > 100) evaluated to false"
//@ diag: 61:3 error "no matching function for call to 'f'"
//@ diag: 56:38 note "candidate template ignored: constraints not satisfied [with X = N::H<int> &]"
//@ diag: 56:28 note "because 'N::H<int> &' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::H<int> &) > 100' (1 > 100) evaluated to false"
//@ diag: 62:3 error "no matching function for call to 'f'"
//@ diag: 56:38 note "candidate template ignored: constraints not satisfied [with X = N::H<int>]"
//@ diag: 56:28 note "because 'N::H<int>' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::H<int>) > 100' (1 > 100) evaluated to false"
//@ diag: 63:3 error "no matching function for call to 'f'"
//@ diag: 56:38 note "candidate template ignored: constraints not satisfied [with X = N::P]"
//@ diag: 56:28 note "because 'N::P' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::P) > 100' (1 > 100) evaluated to false"
//@ diag: 64:3 error "no matching function for call to 'g'"
//@ diag: 57:38 note "candidate template ignored: constraints not satisfied [with X = N::H<int>]"
//@ diag: 57:28 note "because 'N::H<int>' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::H<int>) > 100' (1 > 100) evaluated to false"
//@ diag: 66:3 error "no matching function for call to 'f'"
//@ diag: 56:38 note "candidate template ignored: constraints not satisfied [with X = H<int>]"
//@ diag: 56:28 note "because 'N::H<int>' does not satisfy 'R'"
//@ diag: 55:31 note "because 'sizeof(N::H<int>) > 100' (1 > 100) evaluated to false"
//@ diag: 73:13 error "no matching function for call to 'k'"
//@ diag: 69:24 note "candidate template ignored: deduced conflicting types for parameter 'X' ('N2::H<int>' vs. 'int')"
//@ diag: 73:23 error "no matching function for call to 'm'"
//@ diag: 70:24 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 2nd argument"
//@ diag: 73:32 error "no matching function for call to 'm'"
//@ diag: 70:24 note "candidate function template not viable: no known conversion from 'int' to 'int *' for 2nd argument"
//@ diag: 73:47 error "no matching function for call to 'c'"
//@ diag: 71:51 note "candidate template ignored: constraints not satisfied [with X = N2::P]"
//@ diag: 71:29 note "because 'sizeof(N2::P) > 100' (1 > 100) evaluated to false"
//@ diag: 73:53 error "no matching function for call to 'c'"
//@ diag: 71:51 note "candidate template ignored: constraints not satisfied [with X = N2::P]"
//@ diag: 71:29 note "because 'sizeof(N2::P) > 100' (1 > 100) evaluated to false"
//@ diag: 73:65 error "no matching function for call to 'c'"
//@ diag: 71:51 note "candidate template ignored: constraints not satisfied [with X = N2::P]"
//@ diag: 71:29 note "because 'sizeof(N2::P) > 100' (1 > 100) evaluated to false"
//@ diag: 75:13 error "no matching function for call to 'f8'"
//@ diag: 74:81 note "candidate template ignored: constraints not satisfied [with T = int, $1 = int, U = void]"
//@ diag: 74:59 note "because 'sizeof(int) > 100' (4 > 100) evaluated to false"
//@ diag: 79:13 error "no matching function for call to 'h3'"
//@ diag: 78:25 note "candidate template ignored: deduced conflicting types for parameter 'I' ('int' vs. 's3::al<PC>' (aka 'al<pr<const int, long>>'))"
//@ requires: clang<22
// clang 22 emits error: no matching function for call to 'h3'; WP-355 residual
namespace N { template<class T> struct H {}; struct P {}; }
template<class T> concept R = sizeof(T) > 100;
template<class X> requires R<X> void f(char, X&&);
template<class X> requires R<X> void g(char, X);
N::H<int> h;
void t() {
  f('a', N::H<int>());
  f('a', h);
  f('a', N::H<int>{});
  f('a', N::P());
  g('a', N::H<int>());
  using N::H;
  f('a', H<int>());
}
namespace N2 { template<class T> struct H {}; struct P {}; }
template<class X> void k(X, X);
template<class X> void m(X, int*);
template<class X> requires (sizeof(X) > 100) void c(X);
N2::H<int> h2; N2::P p;
void t2() { k(h2, 1); m(p, 1); m(N2::P(), 1); c(p); c(N2::P()); c(N2::P{}); }
template <class T, class = int, class U = void> requires (sizeof(T) > 100) void f8(T);
void g8() { f8(1); }
namespace s3 { template <class T> struct al {}; template <class A, class B> struct pr {}; }
using PC = s3::pr<const int, long>;
template <class I> void h3(I, I);
void t3() { h3(1, s3::al<PC>()); }
