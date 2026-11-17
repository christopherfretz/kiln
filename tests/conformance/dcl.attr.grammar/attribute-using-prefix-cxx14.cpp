// [dcl.attr.grammar] (P0028R4, C++17): an attribute-using-prefix before
// C++17 is clang's "default scope specifier for attributes is a C++17
// extension" [-Wc++17-extensions] at `using`.
//@ expect: compile-ok
//@ std: c++14
//@ strict-diags
//@ diag: 8:3 warning "default scope specifier for attributes is a C++17 extension"
[[using gnu: unused]] static int a1;
[[gnu::unused]] static int a2;
