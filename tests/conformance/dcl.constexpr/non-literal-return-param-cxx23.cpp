// [dcl.constexpr] (C++23, P2448R2): the return and parameter types of a
// constexpr or consteval function need not be literal types, even under
// `-Winvalid-constexpr` (clang skips `CheckLiteralType` from C++23;
// WP-1123).
//@ expect: compile-ok
//@ std: c++23
//@ args: -Winvalid-constexpr
//@ strict-diags

struct NL { NL(); };
struct D { constexpr D() {} ~D() {} };

constexpr D ret() { return D(); }
constexpr int param(NL, int x) { return x; }
consteval int consteval_param(D, int x) { return x; }
auto lambda = [](NL, int x) constexpr { return x; };
