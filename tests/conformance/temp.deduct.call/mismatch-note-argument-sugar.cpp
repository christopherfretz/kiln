// [temp.deduct.type]/1, [temp.deduct.call]/2-4: the "could not match" note
// names the argument type as written, so a typedef shows with its aka text:
// the argument type unqualified, decayed for a non-reference parameter, or
// the part of it the mismatch is in.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 56:3 error "no matching function for call to 'f'"
//@ diag: 51:24 note "candidate template ignored: could not match 'W<T>' against 'I' (aka 'int')"
//@ diag: 57:3 error "no matching function for call to 'f'"
//@ diag: 51:24 note "candidate template ignored: could not match 'W' against 'V'"
//@ diag: 58:3 error "no matching function for call to 'f'"
//@ diag: 51:24 note "candidate template ignored: could not match 'W<T>' against 'XA' (aka 'n::X')"
//@ diag: 59:3 error "no matching function for call to 'h'"
//@ diag: 53:24 note "candidate template ignored: could not match 'W' against 'V'"
//@ diag: 60:3 error "no matching function for call to 'g'"
//@ diag: 52:24 note "candidate template ignored: could not match 'W' against 'V'"
//@ diag: 71:3 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'I' (aka 'int')"
//@ diag: 71:10 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'I *' (aka 'int *')"
//@ diag: 71:18 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'IP' (aka 'int *')"
//@ diag: 71:25 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'I *' (aka 'int *')"
//@ diag: 71:32 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'int'"
//@ diag: 71:39 error "no matching function for call to 'f'"
//@ diag: 64:24 note "candidate template ignored: could not match 'W<T>' against 'I' (aka 'int')"
//@ diag: 72:3 error "no matching function for call to 'r'"
//@ diag: 65:24 note "candidate template ignored: could not match 'W<T>' against 'const I' (aka 'const int')"
//@ diag: 72:10 error "no matching function for call to 'r'"
//@ diag: 65:24 note "candidate template ignored: could not match 'W<T>' against 'I[3]' (aka 'int[3]')"
//@ diag: 72:18 error "no matching function for call to 'r'"
//@ diag: 65:24 note "candidate template ignored: could not match 'W<T>' against 'IP' (aka 'int *')"
//@ diag: 72:25 error "no matching function for call to 'r'"
//@ diag: 65:24 note "candidate template ignored: could not match 'W<T>' against 'CI' (aka 'const int')"
//@ diag: 73:3 error "no matching function for call to 'p'"
//@ diag: 66:24 note "candidate template ignored: could not match 'W<T>' against 'int'"
//@ diag: 73:10 error "no matching function for call to 'p'"
//@ diag: 66:24 note "candidate template ignored: could not match 'W<T>' against 'I' (aka 'int')"
//@ diag: 73:17 error "no matching function for call to 'p'"
//@ diag: 66:24 note "candidate template ignored: could not match 'W<T>' against 'I' (aka 'int')"
//@ diag: 74:3 error "no matching function for call to 'q'"
//@ diag: 67:24 note "candidate template ignored: could not match 'W<T>' against 'int'"
template<class T> struct W {};
template<class T> struct V {};
using I = int;
using VI = V<int>;
namespace n { struct X {}; }
using XA = n::X;
template<class T> void f(W<T>);
template<class T> void g(T*, W<T>);
template<class T> void h(const W<T>&);
void t() {
  I i; VI v; XA x;
  f(i);
  f(v);
  f(x);
  h(v);
  g(&i, v);
}
using IP = int*;
using CI = const int;
template<class T> void f(W<T>);
template<class T> void r(W<T>&);
template<class T> void p(W<T>*);
template<class T> void q(T*, W<T>);
template<class T> void fw(W<T>&&);
void t2() {
  const I ci = 0; I arr[3]; IP ip = nullptr; I *pi = nullptr; CI cc = 0;
  f(ci); f(arr); f(ip); f(pi); f(cc); f(I{});
  r(ci); r(arr); r(ip); r(cc);
  p(ip); p(pi); p(arr);
  q(pi, 1);
  fw(ci); fw(I{});
}
