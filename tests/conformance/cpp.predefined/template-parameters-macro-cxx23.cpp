// [cpp.predefined]/1: before C++26 `__cpp_template_parameters` is not
// defined. WP-771.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
#ifdef __cpp_template_parameters
#error "__cpp_template_parameters is C++26"
#endif
