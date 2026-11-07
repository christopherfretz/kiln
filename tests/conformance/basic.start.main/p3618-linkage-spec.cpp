// P3618R0 ([basic.start.main]/3, a DR): `main` may be declared in a named
// module's purview inside `extern "C++"`, which attaches it to the global
// module.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
export module M;
extern "C++" int main() { return 0; }
