//@ expect: pp
//@ args: -I%{test_dir}/Inputs
// __has_include and __has_include_next in #if/#elif, with quoted,
// angled and macro-expanded operands, and as an operand of defined.
#if __has_include("Inputs/simple.h")
quoted_found;
#endif
#if __has_include(<simple.h>)
angled_found;
#endif
#if !__has_include("Inputs/does-not-exist.h")
missing_not_found;
#endif
#define HDR "Inputs/other.h"
#if __has_include(HDR)
macro_found;
#endif
#define ANGLED <other.h>
#if __has_include(ANGLED) && defined(__has_include)
macro_angled_found;
#endif
#if 0
#elif __has_include("Inputs/empty.h")
elif_found;
#endif
#ifdef __has_include
has_include_is_defined;
#endif
