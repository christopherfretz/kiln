// [expr.typeid]/5, [basic.type.qualifier]/3: the cv-qualifiers of an array
// type are those of its element type, so they are top-level and typeid
// ignores them: `typeid(S volatile[3][4])` is `typeid(S[3][4])`, named
// `A3_A4_1S`, through a typedef, an expression operand and a template
// argument alike. kiln stripped only the array's own (empty) qualifiers and
// named `A3_A4_V1S` (found running GCC's C++ testsuite). The cv of a
// pointee or of an element behind a pointer is not top-level and stays.
//@ expect: run
//@ stdout: A3_A4_1S
//@ stdout: A3_A4_1S
//@ stdout: A2_i
//@ stdout: A2_1S
//@ stdout: A_i
//@ stdout: A2_PK1S
//@ stdout: PA2_K1S
//@ stdout: A2_i
//@ stdout: 1 1 1
#include <typeinfo>
extern "C" int printf(const char *, ...);
extern "C" int puts(const char *);
struct S {};
typedef S volatile T[4];
T t[3];
const S a[2] = {};
template <class U>
const std::type_info &of() {
  return typeid(U);
}
int main() {
  puts(typeid(S volatile[3][4]).name());
  puts(typeid(t).name());
  puts(typeid(const int[2]).name());
  puts(typeid(a).name());
  puts(typeid(const volatile int[]).name());
  puts(typeid(const S *[2]).name());
  puts(typeid(const S(*)[2]).name());
  puts(of<const int[2]>().name());
  printf("%d %d %d\n", typeid(const int[2]) == typeid(int[2]),
         typeid(t) == typeid(S[3][4]), of<const int[2]>() == typeid(int[2]));
}
