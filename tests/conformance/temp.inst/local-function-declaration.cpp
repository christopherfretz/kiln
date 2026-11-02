// [temp.inst]/3, [basic.scope.block], [class.friend]/11: a block-scope function
// declaration in a function template is declared again in each instantiation
// with the substituted signature, and a local class's friends redeclare it there.
// The local friend checks run once, at the template definition; a substituted
// declaration that conflicts with a namespace function is diagnosed per
// instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:17 error "cannot define friend function in a local class definition"
//@ diag: 29:17 error "cannot define friend function in a local class definition"
//@ diag: 30:17 error "cannot define friend function in a local class definition"
//@ diag: 23:7 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 34:15 note "in instantiation of function template specialization 't<long>' requested here"
//@ diag: 18:6 note "previous declaration is here"
//@ requires: clang>=22
// kiln at -fclang-compat=22 (WP-377); clang 19 wording: "no matching function found in local scope"
struct A {};
long g(long);
template <class T> void t() {
  void f(int);
  void ft(T);
  bool operator==(A, T);
  int g(T);
  struct L {
    friend void f(int);
    friend void ft(T);
    friend bool operator==(A, T);
    friend void h();
    friend void ht(T);
    friend void f(long);
  };
}
template void t<int>();
template void t<long>();
