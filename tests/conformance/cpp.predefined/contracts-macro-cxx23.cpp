// [cpp.predefined]/1: before C++26 `__cpp_contracts` is not defined.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __cpp_contracts
#error "__cpp_contracts is C++26"
#endif
