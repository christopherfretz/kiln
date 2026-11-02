// [dcl.stc]/9: `mutable` applies only to non-static data members. On a
// variable at namespace or block scope, a for/range-for variable, a
// structured binding, a variable template or an out-of-class static data
// member definition clang says "'mutable' can only be applied to member
// variables" at the name and the variable is invalid (its initializer is
// not checked). A parameter gets "invalid storage class specifier in
// function declarator", a function "illegal storage class on function", a
// reference member "'mutable' cannot be applied to references" (WP-327).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:13 error "'mutable' can only be applied to member variables"
//@ diag: 28:27 error "'mutable' can only be applied to member variables"
//@ diag: 29:30 error "'mutable' can only be applied to member variables"
//@ diag: 30:8 error "invalid storage class specifier in function declarator"
//@ diag: 31:1 error "illegal storage class on function"
//@ diag: 33:15 error "'mutable' can only be applied to member variables"
//@ diag: 34:16 error "'mutable' can only be applied to member variables"
//@ diag: 35:20 error "'mutable' can only be applied to member variables"
//@ diag: 37:20 error "'mutable' can only be applied to member variables"
//@ diag: 38:3 error "declaration cannot be declared 'mutable'"
//@ diag: 38:16 error "'mutable' can only be applied to member variables"
//@ diag: 41:3 error "'mutable' cannot be applied to references"
//@ diag: 45:16 error "'mutable' can only be applied to member variables"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).

mutable int x = "s";
namespace N { mutable int y; }
template <class T> mutable T vt = T();
void f(mutable int p);
mutable int g();
void h() {
  mutable int a = "t";
  mutable int &r = a;
  for (mutable int i = 0; i < 1; ++i) {}
  int arr[1]{};
  for (mutable int e : arr) {}
  mutable auto [q] = arr;
}
struct S {
  mutable int &ref;
  mutable int ok;
  static int m;
};
mutable int S::m = 1;
