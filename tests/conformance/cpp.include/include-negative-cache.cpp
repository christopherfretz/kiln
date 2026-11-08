// [cpp.include]/3: a header missing from an earlier search directory is
// found in a later one, however many earlier misses header search cached
// (kiln reads a directory's listing after a few misses there and answers
// later misses from it; spec/pp.md "Header search"): five names only in
// d2/sub, then one in d1/sub (present in the listing), then another only in
// d2/sub; a subdirectory d1 lacks entirely; and repeats of each lookup.
//@ expect: compile-ok
//@ strict-diags
//@ args: -I%{test_dir}/Inputs/search/d1 -I%{test_dir}/Inputs/search/d2
#include <sub/m1.h>
#include <sub/m2.h>
#include <sub/m3.h>
#include <sub/m4.h>
#include <sub/m5.h>
#include <sub/hit.h>
#include <sub/m6.h>
#include <only/x.h>
#include <only/x.h>
#include <sub/m1.h>
#include <sub/hit.h>
#include <sub/other.h>
#if __has_include(<sub/absent.h>) || __has_include(<absent/x.h>)
#error found a header that does not exist
#endif
#if !__has_include(<sub/m6.h>) || !__has_include(<sub/hit.h>)
#error lost a header after a miss
#endif
static_assert(m1 + m2 + m3 + m4 + m5 + m6 == 21);
static_assert(d1_hit == 1 && d2_hit == 2 && only_x == 7 && d1_other == 1);
