//@ expect: pp
//@ pp-linemarkers
//@ args: -isystem tests/differential/pp/torture/Inputs/sys -I tests/differential/pp/torture/Inputs
// Line markers for nested includes, system headers (flag 3), returning to
// the includer, and blank-line runs that clang collapses into a marker.
first;
#include "Inputs/nested-outer.h"
#include <sys-header.h>
#include <simple.h>



after_blank_lines;








after_many_blank_lines;
