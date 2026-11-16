// [expr.const.imm]/2, [dcl.init.general]/7.2: `S e[2];` and `const S i[2];`
// in a function template default-initialize each element by a consteval
// default constructor, an immediate invocation per element; instantiation
// redoes the default-initialization of the array rather than converting
// the pattern's initializer. Found running GCC's C++ testsuite. WP-1509.
//@ expect: run
//@ stdout: 2 2 9
extern "C" int printf(const char *, ...);
struct S {
  consteval S() : a(1), b(2) { a++; }
  consteval S(int x) : a(x), b(x) {}
  int a, b;
};
template <int N> int f() {
  S e[2];
  const S i[2];
  S j[2] = {1, 7};
  return e[1].a * 100 + i[0].a * 10 + j[1].b + 0 * N;
}
int main() {
  int r = f<0>();
  printf("%d %d %d\n", r / 100, r / 10 % 10, r % 10 + 2);
}
