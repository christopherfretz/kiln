// [cpp.predefined]/1 (P2841R7): at -fclang-compat=19 kiln rejects concept
// and variable template template parameters, as clang 19 does, so
// `__cpp_template_parameters` is not defined there. WP-771.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
#ifdef __cpp_template_parameters
#error "__cpp_template_parameters needs -fclang-compat=22"
#endif
