// [dcl.enum]: enumerators without an initializer take the previous value
// plus one; scoped enumerations need an explicit conversion.
//@ expect: run
//@ stdout: 0 1 10 11 -1
//@ stdout: 2 255
extern "C" int printf(const char*, ...);

enum Color { Red, Green, Blue = 10, Cyan, Neg = -1 };
enum class Level : unsigned char { Low = 1, Mid, High = 255 };

int main() {
  printf("%d %d %d %d %d\n", Red, Green, Blue, Cyan, Neg);
  Level l = Level::Mid;
  printf("%d %d\n", static_cast<int>(l), static_cast<int>(Level::High));
}
