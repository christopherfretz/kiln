//@ expect: pp
// #pragma once and include guards, including files that include
// themselves: each guarded body appears once, bounded self-recursion
// unrolls, and an unguarded file appears every time.
#include "Inputs/once.h"
#include "Inputs/once.h"
#include "Inputs/./once.h"
#include "Inputs/guarded.h"
#include "Inputs/guarded.h"
#include "Inputs/recurse.h"
#include "Inputs/unguarded.h"
#include "Inputs/unguarded.h"
#undef TORTURE_GUARDED_H
#include "Inputs/guarded.h"
end;
