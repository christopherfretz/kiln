// [expr.assign]/1: the left operand must be a modifiable lvalue; when it
// is (a non-const member of) a call whose return type is const-qualified,
// clang names the function and notes its return type (libc++
// pair get_const).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:9 error "cannot assign to return value because function 'ref' returns a const value"
//@ diag: 14:7 note "function 'ref' which returns const-qualified type 'const int &' declared here"
//@ diag: 20:13 error "cannot assign to return value because function 'get<0>' returns a const value"
//@ diag: 16:24 note "function 'get<0>' which returns const-qualified type 'const int &' declared here"
//@ diag: 21:12 error "cannot assign to return value because function 'sref' returns a const value"
//@ diag: 15:7 note "function 'sref' which returns const-qualified type 'const S &' declared here"
struct S { int m; };
const int &ref();
const S &sref();
template <int I> const int &get(const S &);
int &plain();
void f(const S &s) {
  ref() = 1;
  get<0>(s) = 5;
  sref().m = 1;
  plain() = 1;
}
