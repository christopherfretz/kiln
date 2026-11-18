// __builtin_has_attribute in C: an integer constant expression of type
// _Bool, usable in _Static_assert; a name asks its declaration, then its
// type; a type asks its typedefs and its tag. Found running GCC's C++
// testsuite (its c-c++-common cases run in both languages). WP-1611.
//@ std: c17
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
typedef int I8 __attribute__((aligned(8)));
struct __attribute__((packed)) P { char c; int i; };
__attribute__((weak)) int w;
I8 i8;
int plain;
void f(void) __attribute__((noreturn));

_Static_assert(__builtin_has_attribute(w, weak), "w");
_Static_assert(!__builtin_has_attribute(plain, weak), "plain");
_Static_assert(__builtin_has_attribute(i8, aligned(8)), "i8");
_Static_assert(__builtin_has_attribute(I8, aligned), "I8");
_Static_assert(__builtin_has_attribute(struct P, packed), "P");
_Static_assert(__builtin_has_attribute(f, noreturn), "f");
_Static_assert(_Generic(__builtin_has_attribute(f, noreturn), _Bool: 1), "_Bool");
int arr[__builtin_has_attribute(i8, aligned) + 1];
