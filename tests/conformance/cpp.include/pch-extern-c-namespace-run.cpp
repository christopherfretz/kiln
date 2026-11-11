// [dcl.link]/6: two declarations of a function or variable with C
// language linkage and the same name declare the same entity whatever
// namespaces they are in, also when the earlier ones are in an included
// header. With `--pch-diff`, the header's precompiled image carries the
// C linkage specification in its namespace (WP-1310).
//@ expect: run
#include "Inputs/extern_c_namespace.h"

extern "C" int ecn_first(int x) { return x + 1; }
namespace other {
extern "C" int ecn_second(int x) { return 2 * x; }
} // namespace other
extern "C" int ecn_var = 10;

int main() {
  ecn::T t{1};
  if (ecn::use(3) != 4 + 6 + 10 || ::ecn_second(1) != 2)
    return 1;
  return ecn::ecn_first(0) == 1 && t.v == 1 ? 0 : 2;
}
