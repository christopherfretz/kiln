// Debug info (WP-1365, [namespace.udir], [namespace.alias]): using-directives
// at namespace scope (DW_TAG_imported_module in the compile unit's
// `imports:`, scoped to the unit or the enclosing namespace), a
// using-directive naming an alias (the namespace itself), namespace
// aliases (DW_TAG_imported_declaration named after the alias; an alias of
// an alias imports the first alias's entity), an inline namespace (an
// ordinary namespace with `exportSymbols`), and the anonymous namespace,
// whose implicit using-directive clang leaves out.
//@ args: -g
namespace outer {
int depth = 1;
namespace inner {
int value = 2;
}
inline namespace v1 {
int version = 3;
}
} // namespace outer
namespace {
int hidden = 4;
}
namespace OI = outer::inner;
namespace OI2 = OI;
using namespace outer;
using namespace OI2;
namespace user {
using namespace outer::inner;
namespace local_alias = outer;
int sum() { return value + local_alias::depth; }
} // namespace user
extern "C++" {
using namespace outer::v1;
}
int main() { return depth + value + version + hidden + OI2::value + user::sum(); }
