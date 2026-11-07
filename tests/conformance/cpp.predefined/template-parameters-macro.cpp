// [cpp.predefined]/1, [tab:cpp.predefined.ft] (P2841R7): with concept and
// variable template template parameters implemented,
// `__cpp_template_parameters` is 202502L in C++26. clang 22 implements the
// paper but does not define the macro (docs/STANDARD_SUPPORT.md "Macro
// gaps"). WP-771.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not define __cpp_template_parameters
//@ requires: clang>=22
static_assert(__cpp_template_parameters == 202502L);
