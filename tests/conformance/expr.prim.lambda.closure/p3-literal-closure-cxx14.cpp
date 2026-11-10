// [expr.prim.lambda.closure]/3 (C++11/14 wording, changed by P0170R1),
// [basic.types]/10, [dcl.constexpr]/3: before C++17 a closure type is not a
// literal type, whatever it captures, so a constexpr function cannot return
// or take one and a constexpr variable cannot have one. clang
// `RequireLiteralType` names the reason first (WP-1225).
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ diag: 29:18 error "constexpr function's return type '(lambda at "
//@ diag: 29:42 note "lambda closure types are non-literal types before C++17"
//@ diag: 30:18 error "constexpr function's return type '(lambda at "
//@ diag: 30:42 note "lambda closure types are non-literal types before C++17"
//@ diag: 31:18 error "constexpr function's return type '(lambda at "
//@ diag: 31:39 note "lambda closure types are non-literal types before C++17"
//@ diag: 32:18 error "constexpr function's return type '(lambda at "
//@ diag: 32:34 note "lambda closure types are non-literal types before C++17"
//@ diag: 33:18 error "constexpr function's return type '(lambda at "
//@ diag: 33:34 note "lambda closure types are non-literal types before C++17"
//@ diag: 34:18 error "constexpr function's return type '(lambda at "
//@ diag: 34:37 note "lambda closure types are non-literal types before C++17"
//@ diag: 37:35 error "constexpr function's 1st parameter type 'decltype(none)' (aka '(lambda at "
//@ diag: 36:13 note "lambda closure types are non-literal types before C++17"
//@ diag: 38:16 error "constexpr variable cannot have non-literal type 'const (lambda at "
//@ diag: 38:22 note "lambda closure types are non-literal types before C++17"

struct NL { NL(); ~NL(); int v; };
struct S {
  int m;
  constexpr auto by_ref(int &r) { return [&r] { return r; }; }
  constexpr auto by_copy(int r) { return [r] { return r; }; }
  constexpr auto init(int r) { return [y = r] { return y; }; }
  constexpr auto self() { return [this] { return m; }; }
  constexpr auto none() { return [] { return 1; }; }
  constexpr auto generic() { return [](auto x) { return x; }; }
};
auto none = [] { return 1; };
constexpr int take(decltype(none) l) { return 1; }
constexpr auto var = [] { return 1; };
