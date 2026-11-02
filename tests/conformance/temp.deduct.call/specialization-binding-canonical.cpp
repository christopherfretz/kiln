// [temp.deduct.call]/1, [temp.arg.explicit]/2, [temp.spec.general]: a
// specialization is identified by its canonical template arguments, so
// a candidate note that names the specialization formed by deduction
// ("candidate function [with ...]") binds the canonical types, whatever
// sugar the call argument (`myint`, `s::pl`) or the explicit template
// argument carried. The deduced arguments of a failed deduction keep
// the argument's sugar (`'myint' (aka 'int')`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:3 error "no matching function for call to 'a'"
//@ diag: 23:25 note "candidate function [with T = int] not viable: no known conversion from 'int' to 'typename id<int>::type *' (aka 'int *') for 2nd argument"
//@ diag: 28:3 error "no matching function for call to 'f'"
//@ diag: 24:34 note "candidate function [with T = s::plain, U = int] not viable: no known conversion from 'int' to 'typename id<int>::type *' (aka 'int *') for 3rd argument"
//@ diag: 29:3 error "no matching function for call to 'r'"
//@ diag: 25:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('myint' (aka 'int') vs. 'long')"
namespace s {
struct plain {};
using pl = plain;
}
template <class T> struct W {};
template <class T> struct id { using type = T; };
typedef int myint;
template <class T> void a(T, typename id<T>::type *);
template <class T, class U> void f(T, U, typename id<U>::type *);
template <class T> void r(T &, int, T);
void h(s::pl x, myint i, W<s::pl> w) {
  a(i, 1);
  f<s::pl>(x, i, 1);
  r(i, 1, 2L);
}
