// [basic.stc.thread], [basic.def.odr]/1: a namespace-scope thread_local
// variable of a module interface has one definition, the module unit's;
// an importer declares it (`external thread_local`) and reaches one with
// a dynamic initializer through its `_ZTW` wrapper (comdat, as for an
// `extern thread_local` declaration) and the module's `_ZTH` init
// function (referenced `extern_weak`), never defining either, so the
// module and two importers link and see one object per thread. clang 22
// defines every such variable and its `_ZTH` alias again in each importer
// (strong definitions the module object also provides: the link fails
// with multiple definitions); kiln does not follow it (WP-1669).
//@ expect: run
//@ std: c++20
//@ module: tlv=Inputs/thread-locals.cppm
//@ unit: Inputs/thread-locals-user.cpp
//@ skip(clang): clang 22 importers redefine the module's thread_local variables (multiple definitions at the link)
//@ stdout: 45 45 13 same 1 1 1
import tlv;
extern "C" int printf(const char *, ...);

int user_sum();
const int *user_c1();

int main() {
  int mine = c1 + c2 + m1 + d1 + dc + cd.v;
  printf("%d %d %d same %d %d %d\n", mine, user_sum(), c1 + d1, &c1 == module_c1(),
         user_c1() == module_c1(), &d1 == module_d1());
}
