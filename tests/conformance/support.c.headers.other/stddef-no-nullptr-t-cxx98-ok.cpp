// [support.c.headers.other]/1: C++98 has no `nullptr_t`, so <stddef.h> does
// not declare `::nullptr_t` there and the name is free for the program.
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++98
#include <stddef.h>
typedef int nullptr_t;
nullptr_t k = 0;
size_t s = sizeof(k);
