// [depr.volatile.type]/2, [temp.deduct.general]/7: a volatile-qualified
// return type formed by substitution is deprecated, at the return type
// as written. A function template specialization formed by deduction
// reports it at its first use (from clang 22 on with the context notes,
// clang 19 without); a class member or alias at
// the instantiation; a non-dependent one only at its definition; a
// deduced `auto` / `decltype(auto)` return type never.
//@ std: c++26
//@ args: -Wno-vexing-parse
//@ requires: clang>=22
//@ strict-diags
//@ diag: 40:44 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 34:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 45:3 note "while substituting deduced template arguments into function template 'f' [with T = volatile int]"
//@ diag: 35:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 46:3 note "while substituting deduced template arguments into function template 'g' [with T = volatile int]"
//@ diag: 36:30 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 47:19 note "in instantiation of template class 'S<volatile int>' requested here"
//@ diag: 37:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 48:3 note "while substituting deduced template arguments into function template 'q' [with T = volatile int]"
//@ diag: 34:19 warning "volatile-qualified return type 'const volatile int' is deprecated"
//@ diag: 49:3 note "while substituting deduced template arguments into function template 'f' [with T = const volatile int]"
//@ diag: 38:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 51:3 note "in instantiation of template type alias 'F' requested here"
//@ diag: 39:30 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 52:5 note "while substituting deduced template arguments into function template 'm' [with U = volatile int]"
//@ diag: 39:62 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 53:3 note "while substituting deduced template arguments into function template 'sm' [with U = volatile int]"
//@ diag: 34:19 warning "volatile-qualified return type 'volatile long' is deprecated"
//@ diag: 41:30 note "while substituting deduced template arguments into function template 'f' [with T = volatile long]"
//@ diag: 58:15 note "in instantiation of function template specialization 'h<volatile long>' requested here"
//@ diag: 40:48 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 54:3 note "in instantiation of function template specialization 'k<volatile int>' requested here"
template<class T> T f();
template<class T> T g() { return T(); }
template<class T> struct S { T m(); };
template<class T> auto q() -> T;
template<class T> using F = T();
struct X { template<class U> U m(); template<class U> static U sm(); };
template<class T> void k() { volatile int r(); T s(); }
template<class T> void h() { f<T>(); }
template<class T> auto a() { return T(); }
template<class T> decltype(auto) b() { static T x; return (x); }
void use(X x) {
  f<volatile int>();
  g<volatile int>();
  S<volatile int> s;
  q<volatile int>();
  f<const volatile int>();
  f<volatile int>();
  F<volatile int> *fp;
  x.m<volatile int>();
  X::sm<volatile int>();
  k<volatile int>();
  a<volatile int>();
  b<volatile int>();
}
template void h<volatile long>();
