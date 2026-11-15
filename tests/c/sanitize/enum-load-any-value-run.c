// C23 6.7.3.3p13-15 (C17 6.7.2.2p4): an enumerated type is compatible
// with an integer type, and an object of it may hold any value of that
// type, not only an enumerator. Under -fsanitize=enum (in `undefined`) a
// load of such a value is valid in C, so clang emits no invalid-value check
// for enumeration loads in C (`getRangeForType`: only a C++ enumeration
// without a fixed underlying type has a narrower range); a `_Bool` load
// is still checked. The program stores and loads values outside the
// enumerators, through the object, a bit-field and a cast, and runs
// clean with recovery off. spec/codegen.md "Sanitizers".
//@ expect: run
//@ requires: sanitizer-runtime
//@ args: -std=gnu17 -fsanitize=undefined -fno-sanitize-recover=all
//@ stdout: 7 -3 3 255 1
int printf(const char *, ...);
enum E { A, B };
enum N { M = -1, P = 1 };
struct S { enum E bf : 4; };
int load(enum E *p) { return *p; }
int main(void) {
  enum E e = (enum E)7;
  enum N n = (enum N)-3;
  struct S s = { (enum E)3 };
  unsigned u = 255;
  _Bool b = 1;
  printf("%d %d %d %d %d\n", load(&e), (int)n, (int)s.bf, *(enum E *)&u, b);
  return 0;
}
