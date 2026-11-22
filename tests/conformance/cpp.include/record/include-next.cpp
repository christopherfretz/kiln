// [cpp.include], clang's `#include_next`: `n.h` found in the first search
// directory includes the next `n.h`, found in the second; each row gives
// the search-path index (spec/pp.md "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-include-next-outside-header -I%{test_dir}/Inputs/next/a -I%{test_dir}/Inputs/next/b
//@ pp-record: include-next.record
#include <n.h>
