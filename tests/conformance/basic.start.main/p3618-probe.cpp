// P3618R0 ([basic.start.main]/3, a DR): `main` is attached to the global
// module. In a named module's purview outside a linkage-specification
// clang 22 warns that it never has module linkage (clang 19 is silent).
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 10:1 warning "'main' never has module linkage"
export module M;
int main() { return 0; }
