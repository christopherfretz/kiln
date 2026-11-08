// [namespace.udir]/1, [namespace.alias]/1: the namespace-name in a
// using-directive or namespace-alias-definition is looked up considering
// only namespaces, so a function of the same name does not hide one.
// Reduced from clang's lib/Index: `using namespace clang; using namespace
// index;` with `char *index(const char *, int)` from <strings.h>.
//@ expect: run
//@ stdout: 3 4 5
#include <cstdio>

extern "C" const char *index(const char *s, int c);

namespace clang {
namespace index {
int x = 3;
}
namespace inner {
namespace index {
int z = 5;
}
} // namespace inner
} // namespace clang

using namespace clang;
using namespace index;

namespace idx = index;
namespace other {
int index(int);
namespace a = clang::index;
} // namespace other

using namespace clang::inner::index;

int main() {
  idx::x += 1;
  std::printf("%d %d %d\n", x - 1, other::a::x, z);
}
