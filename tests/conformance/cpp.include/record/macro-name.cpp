// [cpp.include]/4: a header name from a macro. The name's range is the
// token `H` in the file, and the row names the expansion that produced it
// (spec/pp.md "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: macro-name.record
#define H "Inputs/x.h"
#include H
