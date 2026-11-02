// -Wsystem-headers reports warnings located in system headers (here: the
// lines after a line marker with flag 3, [cpp.line] GNU extension), and
// -Werror then makes them errors (spec/diag.md §5.4).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wsystem-headers -Werror -isystem %{test_dir}/Inputs
//@ diag: 10 error "multi-character character constant"
//@ diag: 11 error "not_an_attribute' ignored"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
# 10 "tests/conformance/cpp.include/system-header-warnings-shown.cpp" 3
int marked_system = 'gh';
[[gnu::not_an_attribute]] int marked_attr;
#include <noisy_system.h>
