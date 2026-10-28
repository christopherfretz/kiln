//@ args: -I%{test_dir}/Inputs -I%{repo_root}/xtask/fixtures/basic -resource-dir %{resource_dir}
// `args:` placeholders are substituted before the compiler runs.
#include <aux.h>
#include <helper.h>
#include <stddef.h>
static_assert(AUX_VALUE == 7 && HELPER_VALUE == 5 && sizeof(size_t) > 0);
