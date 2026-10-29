// Warnings located in system headers are suppressed (clang's
// SuppressSystemWarnings; spec/diag.md §5.4): a header found through
// -isystem, the rest of a header after `#pragma GCC system_header`, and the
// lines after a line marker with flag 3 ([cpp.line], GNU extension). -Werror
// does not resurrect them. (A system macro expanded in a user file warns
// there: kiln_driver's system_header_warnings_like_clang.)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Werror -isystem %{test_dir}/Inputs
#include <noisy_system.h>
#include "Inputs/pragma_system.h"
# 15 "tests/conformance/cpp.include/system-header-warnings.cpp" 3
int marked_system = 'gh';
[[gnu::not_an_attribute]] int marked_attr;
# 18 "tests/conformance/cpp.include/system-header-warnings.cpp"
static_assert(NOISY == 2);
static_assert(pragma_user_part == 1);
static_assert(sizeof(marked_system) == sizeof(int));
