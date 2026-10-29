//@ expect: ast
// [enum.udecl]: a using-enum-declaration introduces the enumerator names
// of the named enumeration as if by a using-declaration for each.
enum class Color { Red, Green = 4 };
enum Fruit { Apple };
typedef Color Paint;
namespace N {
using enum Color;
}
int a = (int)N::Green;
int f() {
  using enum Paint;
  return (int)Red;
}
int g() {
  using enum Fruit;
  return Apple;
}
