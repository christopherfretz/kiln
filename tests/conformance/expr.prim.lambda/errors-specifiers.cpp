// [expr.prim.lambda.general]/4: each decl-specifier may appear at most once;
// `mutable` and `static` are mutually exclusive, and so are `constexpr` and
// `consteval`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:25 error "'mutable' cannot appear multiple times in a lambda declarator"
//@ diag: 11:17 error "lambda cannot be both mutable and static"
//@ diag: 12:25 error "cannot combine with previous 'constexpr' declaration specifier"
void f() {
  auto c = []() mutable mutable {};
  auto d = []() static mutable {};
  auto e = [] constexpr consteval {};
}
