// [cpp.predefined]/1: before C++26 `__cpp_trivial_union` is not defined.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __cpp_trivial_union
#error "__cpp_trivial_union is C++26"
#endif
