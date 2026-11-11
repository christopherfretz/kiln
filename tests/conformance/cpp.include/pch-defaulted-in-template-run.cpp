// [except.spec]/13, [dcl.fct.def.default], [temp.res.general]: an
// explicitly defaulted default constructor named only inside a template
// definition (`tag{}` in `make`'s body, as libc++'s <execution> does)
// keeps its exception specification unevaluated at the end of the
// header. With `--pch-diff check`, the header's image must leave it
// unevaluated too; the importer once evaluated it for every referenced
// defaulted member, which -fkiln-pch-image=check reported as a
// difference from replay (WP-1300).
//@ expect: run
#include "Inputs/defaulted_in_template.h"

static_assert(noexcept(tag{}));

int main() { return make<pol>() == 3 ? 0 : 1; }
