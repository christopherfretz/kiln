// [expr.call]/7: whether a parameter object's lifetime ends when the
// function returns or at the end of the enclosing full-expression is
// implementation-defined; clang (and kiln) end it on return, so a pointer
// to a parameter escaping its call dangles ([expr.const]/10.8,
// [basic.life]/7). A class or a scalar parameter alike (WP-1018).
// Same at -fclang-compat=19 and 22.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-return-stack-address
//@ diag: 23:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 23:19 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 24:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 24:19 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 25:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 25:19 note "read of object outside its lifetime is not allowed in a constant expression"
struct Q { int n = 0; };
struct D { int n = 1; constexpr ~D() {} };
constexpr const Q *escape(Q q) { return &q; }
constexpr const int *escape_int(int q) { return &q; }
constexpr const D *escape_dtor(D d) { return &d; }

constexpr int a = escape({})->n;
constexpr int b = *escape_int(1);
constexpr int c = escape_dtor({})->n;
