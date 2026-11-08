// [cpp.include]/3, GNU #include_next: each #include_next resumes the search
// after the directory that found the current file, so <n.h> through
// -I a -I b -I c enters a/n.h, b/n.h and c/n.h in that order, every time
// (the second chain is answered from header search's per-name cache).
// __FILE__ and the line markers spell each file as directory + name.
//@ expect: pp
//@ pp-linemarkers
//@ args: -Itests/conformance/cpp.include/Inputs/search/next/a -Itests/conformance/cpp.include/Inputs/search/next/b -Itests/conformance/cpp.include/Inputs/search/next/c
#include <n.h>
#include <n.h>
