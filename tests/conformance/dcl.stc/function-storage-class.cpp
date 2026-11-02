// [dcl.stc]/1,4-5: `mutable` applies only to non-static data members and
// `register` to nothing in C++17. Outside a member declaration (namespace or
// block scope, an out-of-class member definition) a function with either is
// invalid (clang "illegal storage class on function"); a redeclaration of an
// invalid function is invalid without a diagnostic, an invalid declaration
// redeclares nothing, and a call through only invalid declarations is
// resolved against the last one. In a member declaration the specifier is
// diagnosed and the member stays valid (WP-288).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:1 error "illegal storage class on function"
//@ diag: 30:1 error "illegal storage class on function"
//@ diag: 35:1 error "illegal storage class on function"
//@ diag: 36:6 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 34:5 note "previous declaration is here"
//@ diag: 39:1 error "illegal storage class on function"
//@ diag: 40:6 error "'s' is missing exception specification 'noexcept'"
//@ diag: 38:6 note "previous declaration is here"
//@ diag: 45:3 error "storage class specified for a member declaration"
//@ diag: 46:3 error "'mutable' cannot be applied to functions"
//@ diag: 48:1 error "illegal storage class on function"
//@ diag: 49:1 error "illegal storage class on function"
//@ diag: 51:1 error "illegal storage class on function"
//@ diag: 53:3 error "illegal storage class on function"
//@ diag: 55:3 error "no matching function for call to 'h'"
//@ diag: 51:14 note "candidate function not viable: requires 1 argument, but 0 were provided"

mutable int f();
long f();
register void g() noexcept;
void g();
static_assert(noexcept(g()));

int r();
mutable int r();
long r();

void s() noexcept;
mutable void s();
void s();

struct S {
  void a();
  void b();
  register void c();
  mutable void d();
};
mutable void S::a() {}
register void S::b() {}

mutable void h(int);
void use() {
  mutable void loc();
  h(1);
  h();
}
