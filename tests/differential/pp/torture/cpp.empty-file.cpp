//@ expect: pp
// Including an empty file and a file that contains only directives and
// comments contributes no tokens.
#include "Inputs/empty.h"
#include "Inputs/directives-only.h"
DIRECTIVES_ONLY
