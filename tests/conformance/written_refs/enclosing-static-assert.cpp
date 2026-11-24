// The written-reference table (spec/ast.md §18.3): a static_assert
// declaration ([dcl.pre]/10) holds the names of its condition and its
// message, though it declares no name: at namespace, class and block
// scope and in a template. A lambda in its condition keeps its call
// operator.
//@ written-refs: enclosing-static-assert.refs
namespace lib {
struct Shape { int w; };
constexpr int base = 4;
} // namespace lib

static_assert(sizeof(lib::Shape) == lib::base);
static_assert([] { return lib::base; }() == 4, "lambda");

struct Holder {
  static_assert(sizeof(lib::Shape::w) == lib::base);
};

template <class T> struct Table {
  static_assert(sizeof(T) >= sizeof(lib::Shape));
};

int local() {
  static_assert(lib::base > 0);
  return lib::base;
}
