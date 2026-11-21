// [basic.stc.thread], [basic.def.odr]/14, [temp.inst]/4, spec/modules.md
// §11.11 "Part 2a" (WP-1824): inline and templated thread_local
// variables of a module (an inline variable, an inline `__thread` one,
// an inline static data member, a class template's static data member,
// a variable template, local statics of a function template and of an
// inline function) are one object per thread across the module and two
// importers, each of which defines them `linkonce_odr` with its own
// `_ZTH` alias and `__tls_init`. Under `--bmi-image` the importers build
// the module from its declaration image, with the same IR.
//@ expect: run
//@ std: c++20
//@ requires: clang>=22
//@ module: imgtli=Inputs/img-thread-local-inline.cppm
//@ unit: Inputs/img-thread-local-inline-user.cpp
//@ stdout: 27 29 1
import imgtli;
extern "C" int printf(const char *, ...);
int user_sum();
int *user_inl();

int main() {
  int a = inl + st + S::si + Box<int>::v + vt<int> + bump<int>() + counter();
  int b = user_sum();
  printf("%d %d %d\n", a, b, user_inl() == addr() && &inl == addr());
}
