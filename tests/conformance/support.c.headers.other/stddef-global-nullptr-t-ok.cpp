// [support.c.headers.other]/1: <stddef.h> declares in the global namespace
// what <cstddef> declares in namespace std, so `::nullptr_t` is
// `decltype(nullptr)` after `#include <stddef.h>` from C++11 on (GCC's
// stddef.h does this; clang's resource stddef.h does not, found running GCC's
// C++ testsuite); <cstddef> after it still compiles.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang's resource stddef.h declares no ::nullptr_t (GCC's does)
#include <stddef.h>
#include <cstddef>
::nullptr_t n = nullptr;
nullptr_t m = n;
template<class T, class U> struct same { static const bool value = false; };
template<class T> struct same<T, T> { static const bool value = true; };
static_assert(same<::nullptr_t, decltype(nullptr)>::value, "");
static_assert(same<::nullptr_t, std::nullptr_t>::value, "");
