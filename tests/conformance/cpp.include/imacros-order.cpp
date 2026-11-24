// [cpp.include], [cpp.replace]: `-imacros <file>` reads the file for its
// macros only: its directives run (an `#include` in it too), its text is
// discarded. Every `-imacros` comes before every `-include`, whatever
// their order on the command line, so an `-include` and the main file see
// the `-imacros` macros (clang AddImplicitIncludeMacros). `--imacros=` is
// the same option. WP-2822.
//@ expect: pp
//@ args: -include tests/conformance/cpp.include/Inputs/imacros/inc.h -imacros tests/conformance/cpp.include/Inputs/imacros/m1.h --imacros=tests/conformance/cpp.include/Inputs/imacros/m2.h
int from_main = M1 + M2 + NESTED;
int seen = INC_SEEN;
