// The written-reference table (spec/ast.md §18.3): an enumerator
// ([dcl.enum]/1) holds the names of its value and its attributes, in a
// named, an unnamed, a scoped and a member enumeration, and in a member
// enumeration of a class template ([temp.mem.enum]).
//@ written-refs: enclosing-enumerator.refs
namespace lib {
struct Shape { int w; };
constexpr int base = 4;
} // namespace lib

enum Color { red = sizeof(lib::Shape), green = lib::base, blue };
enum { anon = lib::base + 1 };
enum class Scoped : int { first [[deprecated]] = lib::base, second };

struct Holder {
  enum Kind { small = sizeof(lib::Shape::w) };
};

template <class T> struct Table {
  enum Size { entries = sizeof(T) + lib::base };
};
