// [cpp.cond]/4, clang's `__has_include_next`: in `hn.h`, found in the
// first search directory, `__has_include_next(<hn.h>)` finds the `hn.h`
// of the second; the query row gives its file and search-path index, as
// the `#include_next` row after it does (spec/pp.md "Preprocessing
// record", queries).
//@ expect: compile-ok
//@ strict-diags
//@ args: -I%{test_dir}/Inputs/next/a -I%{test_dir}/Inputs/next/b
//@ pp-record: has-include-next.record
#include <hn.h>
