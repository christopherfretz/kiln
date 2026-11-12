// [basic.fundamental] (GNU extension: complex types): a header's complex
// type is the same type through a precompiled header, so converting it to
// a class still fails (WP-1320).
//@ expect: compile-fail
//@ diag: 9:3 error "no viable conversion from '_Complex float' to 'S'"
#include "Inputs/complex_types.h"
struct S {};

S s = cxt::make(1, 2);
