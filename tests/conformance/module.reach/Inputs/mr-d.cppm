// Module for ../missing-include-dirs.cpp: global module fragment headers
// found through a relative and an absolute search directory, and one
// included through a macro.
module;
#include "mr-r.h"
#include "mr-a.h"
#include "mr-m.h"
export module mr.d;
export int d_fn();
