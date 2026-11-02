// Module for ../missing-include.cpp: global module fragment declarations
// in an include-guarded header, a header it includes, a `#pragma once`
// header, an unguarded header and a guarded system header.
module;
#include "mr-g.h"
#include "mr-o.h"
#include "mr-n.h"
#include <mr-s.h>
export module mr.h;
export int h_fn();
