// P3868R1 ([cpp.pre], a DR): a #line directive may precede the
// `module;` that opens a global module fragment.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
#line 10 "probe.cpp"
module;
export module M;
export int f() { return 1; }
