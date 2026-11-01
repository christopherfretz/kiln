// GNU `__null`: a null pointer constant of an integer type as wide as a
// pointer (`long` on LP64), which libstdc++'s <cstddef> NULL expands to via
// the compiler's <stddef.h>; first hit at bits/gthr-default.h:676
// (`__gthrw_(pthread_key_create) != __null`), reached from <iomanip> and most
// headers that include <bits/gthr.h> (docs/status/libstdcxx.md, WP-188).
// Verified against clang 19.
//@ expect: compile-ok
//@ xfail(kiln): use of undeclared identifier '__null'
int *p = __null;
void (*fp)() = __null;
static_assert(sizeof(__null) == sizeof(void *));
bool b(int *q) { return q != __null; }
constexpr long n = __null;
static_assert(n == 0);
