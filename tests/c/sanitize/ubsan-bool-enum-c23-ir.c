// -fsanitize=bool,enum in C23: an enumeration with a fixed underlying
// type is not checked either (C23 6.7.3.3p13: its values are those of
// the underlying type), except that one whose underlying type is `bool`
// has a boolean representation and gets the `bool` check (clang
// `hasBooleanRepresentation`). C23 prints `_Bool` as `bool` in the type
// descriptor. spec/codegen.md "Sanitizers".
//@ expect: ir
//@ args: -std=gnu23 -fsanitize=bool,enum
enum E { A, B };
enum F : long { F0 = 3 };
enum G : bool { G0 };
int f1(enum E *p) { return *p; }
bool f2(bool *p) { return *p; }
long f3(enum F *p) { return *p; }
bool f4(enum G *p) { return *p; }
int f5(void *p) { return *(enum F *)p + *(enum G *)p; }
