// [basic.start.main]/3 (P3618R0): `main` attaches to the global module and
// clang 22 warns when it is declared `extern "C"` (-Wmain, at the name),
// on every declaration. clang 19 is silent (WP-1040).
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:18 warning "'main' should not be 'extern"
//@ diag: 11:16 warning "'main' should not be 'extern"
extern "C" { int main(); }
extern "C" int main() { return 0; }
