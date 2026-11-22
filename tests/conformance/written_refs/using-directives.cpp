// The written-reference table (spec/ast.md §18.4, stage 3): the
// namespace a using-directive nominates (`using-directive`,
// [namespace.udir]/1) and the target of a namespace alias (`ns-alias`,
// [namespace.alias]/1), each after its qualifier's `nns` entries. A
// namespace alias written there names the alias; the alias's own name is
// a declaration and has no entry. Both look up namespace names only, so
// the function `outer` does not hide the namespace in `hides`. An
// alias's names are enclosed by the alias; a directive declares nothing
// with a name, so its names are enclosed by the declaration around it
// (§18.3).
//@ written-refs: using-directives.refs
namespace outer {
namespace inner {
int value;
}
inline namespace v1 {
int versioned;
}
} // namespace outer
namespace short_name = outer::inner;
namespace again = short_name;
namespace from_global = ::outer;

using namespace outer;
using namespace ::outer::inner;
using namespace short_name;

int read() {
  using namespace again;
  return value + versioned;
}

namespace hides {
void outer();
using namespace outer;
} // namespace hides

namespace user {
using namespace from_global::inner;
namespace local = from_global::v1;
}
