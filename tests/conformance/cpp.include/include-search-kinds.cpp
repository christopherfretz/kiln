// [cpp.include]/2-3: "k.h" searches the -iquote directories, then -I, then
// -isystem; <k.h> skips the -iquote ones. A header found in an -isystem
// directory is a system header (line-marker flag 3), including one reached
// by #include_next from a user directory.
//@ expect: pp
//@ pp-linemarkers
//@ args: -iquote tests/conformance/cpp.include/Inputs/search/quote -Itests/conformance/cpp.include/Inputs/search/user -isystem tests/conformance/cpp.include/Inputs/search/sys
#include "k.h"
#include <k.h>
#include "s.h"
