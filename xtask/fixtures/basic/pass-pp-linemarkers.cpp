//@ expect: pp
//@ pp-linemarkers
// Line markers are compared as (file, line, flags), not as text: the golden
// spells them differently from clang but means the same.
#include "Inputs/aux.h"


int after;
