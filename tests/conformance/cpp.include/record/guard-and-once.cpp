// [cpp.include]: a header with an include guard and a `#pragma once` header,
// each included twice. The first inclusions are entered and the guard's
// `#ifndef` is flagged as the guard kiln recognized; the second ones are
// skipped by the guard (naming its definition) and by once (spec/pp.md
// "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: guard-and-once.record
#include "Inputs/guarded.h"
#include "Inputs/once.h"
#include "Inputs/guarded.h"
#include "Inputs/once.h"
