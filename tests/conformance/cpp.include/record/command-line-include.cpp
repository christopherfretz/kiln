// [cpp.include], `-include`: the command-line inclusion is a row of kind
// `-include` keyed by its ordinal, not by an offset in the predefines; the
// file it enters is an instance like any other (spec/pp.md "Preprocessing
// record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ args: -include %{test_dir}/Inputs/pre.h
//@ pp-record: command-line-include.record
static_assert(PRE == 1);
#include "Inputs/x.h"
