// [dcl.pre], [temp.res.general]/6: a header template's definition-time
// warning (`-Wunused-variable`) is the header's, reported once by its
// producer. Under `--pch-diff check`, the image's consumer does not repeat
// it while the replay does, and the two runs still agree (WP-1320).
//@ expect: run
//@ args: -Wall
#include "Inputs/defn_warning.h"

int main() { return pick(0) + pick(3) == 3 ? 0 : 1; }
