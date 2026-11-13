// Debug info (WP-1365, [namespace.udecl], [enum.udecl]): using-declarations
// at namespace scope. One DW_TAG_imported_declaration per declaration, for
// the most recently declared entity the name finds (clang `EmitUsingDecl`):
// a function's definition when the unit emits it (before or after the
// using-declaration), else a declaration-only subprogram; a variable's
// definition, else a declaration (`isDefinition: false`); a type. Nothing
// when that entity is a function template, an enumerator (also through
// `using enum`), a class template, or a function whose return type is
// still to be deduced; a using-declaration of a shadow names the entity
// (an `extern "C"` function, here without a linkage name). A declared
// `T x[]` is described as `T[1]`.
//@ args: -g
extern "C" int c_function(int);
namespace lib {
using ::c_function;
int counter = 1;
extern int declared_only;
extern int unsized[];
extern int defined_later;
static int internal = 5;
int twice(int x) { return 2 * x; }
double twice(double x) { return 2 * x; }
int never_defined(int);
int after(int);
void overloaded(long);
template <class T> void overloaded(T);
template <class T> void tmpl_last(T);
void tmpl_last(char);
[[noreturn]] void stop();
int variadic(int, ...);
struct Point {
  int x, y;
};
enum Color { red, green };
enum class Mode { on, off };
typedef unsigned long size_type;
using byte_type = unsigned char;
template <class T> struct Box {
  T t;
};
auto deduced() { return 1; }
auto undeduced();
} // namespace lib
using lib::counter;
using lib::declared_only;
using lib::unsized;
using lib::c_function;
using lib::defined_later;
using lib::internal;
using lib::twice;
using lib::never_defined;
using lib::after;
using lib::overloaded;
using lib::tmpl_last;
using lib::stop;
using lib::variadic;
using lib::Point;
using lib::Color;
using lib::red;
using enum lib::Mode;
using lib::size_type;
using lib::byte_type;
using lib::Box;
using lib::deduced;
using lib::undeduced;
namespace again {
using lib::counter;
}
using again::counter;
namespace lib {
int defined_later = 3;
int after(int v) { return v + 1; }
auto undeduced() { return 2; }
} // namespace lib
int main() {
  Point p{1, 2};
  Color c = red;
  size_type n = 3;
  byte_type b = 4;
  return counter + p.x + c + int(n) + b + twice(1) + after(2) + deduced() + internal;
}
