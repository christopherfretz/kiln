// [dcl.spec.auto.general]/11: a variable with an undeduced placeholder
// type cannot be named in its own initializer, even unevaluated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "variable 'a' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 11:11 error "variable 'p' declared with deduced type 'auto *' cannot appear in its own initializer"
//@ diag: 12:27 error "variable 'd' declared with deduced type 'decltype(auto)' cannot appear in its own initializer"
//@ diag: 14:12 error "variable 'x' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 15:19 error "variable 'r' declared with deduced type 'const auto &' cannot appear in its own initializer"
auto a = a;
auto *p = p;
decltype(auto) d = sizeof(d);
void f() {
  auto x = x + 1;
  const auto &r = r;
}
int ok = 1;
auto b = ok, c = b;
