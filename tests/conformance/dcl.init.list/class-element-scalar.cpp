// [dcl.init.list]/3.9: a single class-type element of a list for a
// non-class type initializes it directly (clang: core-24034, Sema
// TryListInitialization). A failure is reported at the entity — the
// variable, the type of `T{e}`, or the `{` of a parameter or return value —
// not at the element, with no braced-scalar warning; the user conversion's
// result must not narrow; an argument names the element's conversion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:12 error "no viable conversion from 'N' to 'int'"
//@ diag: 41:12 note "passing argument to parameter here"
//@ diag: 43:7 error "no viable conversion from 'N' to 'int'"
//@ diag: 44:7 error "no viable conversion from 'X' to 'int'"
//@ diag: 38:21 note "explicit conversion function is not a candidate"
//@ diag: 45:7 error "conversion function from 'Ex' to 'int' invokes a deleted function"
//@ diag: 37:13 note "'operator int' has been explicitly marked deleted here"
//@ diag: 46:7 error "conversion function from 'Ex' to 'int' invokes a deleted function"
//@ diag: 37:13 note "'operator int' has been explicitly marked deleted here"
//@ diag: 47:9 error "non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list"
//@ diag: 47:9 note "insert an explicit cast to silence this issue"
//@ diag: 48:12 error "non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list"
//@ diag: 48:12 note "insert an explicit cast to silence this issue"
//@ diag: 49:9 error "no viable conversion from 'N' to 'int'"
//@ diag: 50:13 error "non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list"
//@ diag: 50:13 note "insert an explicit cast to silence this issue"
//@ diag: 51:14 error "no viable conversion from 'N' to 'int'"
//@ diag: 52:16 error "no viable conversion from 'N' to 'int'"
//@ diag: 53:5 error "conversion function from 'Ex' to 'int' invokes a deleted function"
//@ diag: 37:13 note "'operator int' has been explicitly marked deleted here"
//@ diag: 40:11 note "passing argument to parameter here"
//@ diag: 54:3 error "no matching function for call to 'f'"
//@ diag: 40:6 note "candidate function not viable: no known conversion from 'N' to 'int' for 1st argument"
//@ diag: 55:6 error "non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list"
//@ diag: 55:6 note "insert an explicit cast to silence this issue"
//@ diag: 56:8 error "cannot initialize a variable of type 'bool' with an rvalue of type 'std::nullptr_t'"
//@ diag: 57:10 error "no viable conversion from returned value of type 'N' to function return type 'int'"
struct N {};
struct Ex { operator int() = delete; };
struct X { explicit operator int(); };
struct L { operator long(); };
void f(int);
void h(int = {N{}});
int t(N n, Ex e, X x, L l) {
  int a{n};
  int b = {x};
  int c{e};
  int d{(e)};
  int g{l};
  int q = {l};
  (void)int{n};
  (void)int{l};
  int i; i = {n};
  int* p = new int{n};
  f({e});
  f({n});
  f({l});
  bool z = {nullptr};
  return {n};
}
