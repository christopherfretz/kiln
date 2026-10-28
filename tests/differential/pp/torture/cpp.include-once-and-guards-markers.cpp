//@ expect: pp
//@ pp-linemarkers
// The same includes in line-marker mode: the sequence of include enters
// and leaves (and every token's presumed location) must match.
#include "Inputs/once.h"
#include "Inputs/once.h"
#include "Inputs/guarded.h"
#include "Inputs/guarded.h"
#include "Inputs/recurse.h"
#include "Inputs/unguarded.h"
end;
