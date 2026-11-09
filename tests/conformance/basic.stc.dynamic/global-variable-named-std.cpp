// [basic.stc.dynamic.general]/2: the implicit declarations of the global
// allocation functions do not introduce the name std; a later global 'int
// std' is valid. From clang's
// CXX/basic/basic.stc/basic.stc.dynamic/p2-nodef.cpp. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the implicitly declared namespace std is visible to name lookup
//@ expect: compile-ok
int *use_new(int N) { return new int[N]; }
int std = 17;
