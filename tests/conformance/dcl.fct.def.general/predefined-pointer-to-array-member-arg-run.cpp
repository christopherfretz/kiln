// [dcl.fct.def.general]/8, clang `printTemplateArgumentList`: a pointer
// template argument to an array member of a const object prints as the
// array (`&fake.c`), not its first scalar (`&fake.c[0][0]`). The pointee
// `const int[2][2]` and the member `int[2][2]` are the same type up to the
// cv of their elements. reflect-cpp reads field names this way (its C
// arrays tests).
//@ expect: run
//@ strict-diags
//@ stdout: f() [p = W<const int (*)[3]>{&fake.a}]
//@ stdout: f() [p = W<const int (*)[2][2]>{&fake.c}]
//@ stdout: f() [p = &fake.c]
//@ stdout: f() [p = &fake.u]
//@ stdout: f() [p = &fake.c[1]]
//@ stdout: f() [p = &fake.u[1]]
//@ stdout: f() [p = &fake.s]
extern "C" int printf(const char*, ...);
struct S { int x; int y; };
struct T5 { int a[3]; int c[2][2]; S u[2]; S s; };
extern const T5 fake;
template <class T> struct W { T v; };
template <auto p> const char* f() { return __PRETTY_FUNCTION__ + 12; }
int main() {
  const char* r[] = {f<W{&fake.a}>(), f<W{&fake.c}>(), f<&fake.c>(), f<&fake.u>(),
                     f<&fake.c[1]>(), f<&fake.u[1]>(), f<&fake.s>()};
  for (const char* s : r) printf("%s\n", s);
}
