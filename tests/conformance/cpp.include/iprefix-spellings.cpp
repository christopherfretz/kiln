// [cpp.include]/2: the spellings clang accepts: `-iprefix`,
// `-iwithprefix`, `-iwithprefixbefore` and `-imacros` joined or separate,
// `--imacros <f>`, and the GCC aliases `--include-prefix`,
// `--include-with-prefix[-after]` and `--include-with-prefix-before`, each
// separate or `=`-joined. A later `-iprefix` replaces the earlier one.
// WP-2822.
//@ expect: pp
//@ args: -iprefixtests/conformance/cpp.include/Inputs/prefix/ -iwithprefixbeforepb --include-prefix=tests/conformance/cpp.include/Inputs/ --include-with-prefix-after=prefix/pa --include-prefix tests/conformance/cpp.include/Inputs/prefix/ --include-with-prefix-before i -imacrostests/conformance/cpp.include/Inputs/imacros/m1.h --imacros tests/conformance/cpp.include/Inputs/imacros/m2.h
#include <order.h>
#include <before.h>
#include <after.h>
int m = M1 + M2 + NESTED;
