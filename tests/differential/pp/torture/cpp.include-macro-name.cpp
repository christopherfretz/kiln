//@ expect: pp
//@ args: -I%{test_dir}/Inputs
// [cpp.include]/4: a #include whose operand is not a header-name is macro
// replaced, and the result must be a "q-char-sequence" string or a
// <h-char-sequence> built from tokens.
#define QUOTED "Inputs/simple.h"
#include QUOTED
#define ANGLED <other.h>
#include ANGLED
#define STR(x) #x
#define XSTR(x) STR(x)
#include XSTR(Inputs/simple.h)
#define HDR(name) <name.h>
#include HDR(other)
// Tokens of an angled name are spelled back with their whitespace, so
// the pieces here must be adjacent.
#define OPEN <other
#include OPEN.h>
end;
