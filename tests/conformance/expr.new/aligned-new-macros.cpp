// The aligned-allocation flags drive the predefined macros as in
// clang 22: -faligned-new=32 sets __STDCPP_DEFAULT_NEW_ALIGNMENT__ to
// 32 and keeps __cpp_aligned_new; -fnew-alignment=0 restores the
// target default. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++17
//@ args: -faligned-new=32
//@ strict-diags
static_assert(__STDCPP_DEFAULT_NEW_ALIGNMENT__ == 32, "");
static_assert(__cpp_aligned_new == 201606L, "");
