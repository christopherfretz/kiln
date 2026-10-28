// [cpp.include], [cpp.pragma]: a header marked '#pragma once' is entered only
// once, so including it twice does not redefine its contents.
//@ expect: compile-ok
//@ strict-diags
#include "Inputs/once.h"
#include "Inputs/once.h"
#include "Inputs/../Inputs/once.h"
constexpr Once o{once_value};
static_assert(o.value == 3);
