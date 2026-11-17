// [dcl.attr.grammar]/3: the attribute-list after an attribute-using-prefix
// shall not contain an attribute-scoped-token (clang 22 "attribute with
// scope specifier cannot follow default scope specifier" at the scope,
// which is kept; GCC 16.1 rejects it too). Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ std: c++17
//@ requires: clang>=22
//@ strict-diags
//@ diag: 13:13 error "attribute with scope specifier cannot follow default scope specifier"
//@ diag: 13:13 warning "unknown attribute 'ns2::a' ignored"
//@ diag: 14:22 error "attribute with scope specifier cannot follow default scope specifier"
[[using ns: ns2::a]] int a1;
[[using gnu: unused, gnu::unused]] int a2;
[[using gnu: unused]] int a3;
