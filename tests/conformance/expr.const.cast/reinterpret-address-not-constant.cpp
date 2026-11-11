// [expr.const]/5.15: a reinterpret_cast, or a C-style cast performing one, is
// not a core constant expression, so a `constexpr` or `constinit` variable
// may not be initialized with an address cast to an integer or to another
// pointer type, though codegen folds the same initializer of a variable that
// is neither into static data (WP-1295, tests/codegen/ptr_cast_static_init.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 17:20 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 18:17 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 18:21 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 19:16 error "variable does not have a constant initializer"
//@ diag: 19:1 note "required by 'constinit' specifier here"
//@ diag: 19:20 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
using iptr = __INTPTR_TYPE__;
int g;
constexpr iptr a = reinterpret_cast<iptr>(&g);
constexpr char *b = (char *)&g;
constinit iptr c = (iptr)&g;
