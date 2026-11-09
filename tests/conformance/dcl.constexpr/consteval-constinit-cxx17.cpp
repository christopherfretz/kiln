// [dcl.constexpr]/1, [dcl.constinit]/1, [diff.cpp17.lex]/2: before C++20
// 'consteval' and 'constinit' are identifiers, so as decl-specifiers they are
// unknown type names (clang also suggests 'constexpr' for 'consteval'; WP-1134).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 9:1 error "unknown type name 'constinit'"
//@ diag: 10:1 error "unknown type name 'consteval'"
constinit int x = 1;
consteval int f() { return 1; }
