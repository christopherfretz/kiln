// [temp.arg.nontype], [module.interface], spec/modules.md §11.11 Part 2
// (WP-1047): an importer uses constexpr pointers, references and member
// pointers the module evaluated, and specializations keyed on them. A
// kind-2 module file carries those values (an entity plus a byte
// offset) and keys the specializations by them, so the importer sees the
// same addresses and the same specializations as the module (one
// `X<&g>::x`, `XR<g>::x`, `HW<h>::x`), as with kind 1 (`--bmi-kind 1|2`).
// (The template parameter object `&H` of `HW<h>` is one object across
// units too, [temp.param]/8; clang 22 gives each unit its own, so that
// check is in carried-reflections-run.cpp, which has no oracle.)
//@ expect: run
//@ std: c++20
//@ module: cv=Inputs/cv.cppm
//@ stdout: 1 30 8 5 40 14 1
import cv;
extern "C" int printf(const char *, ...);

static_assert(pg == &g && pa == &arr[2] && pend == arr + 4 && psb == &s.b && pn == nullptr);
static_assert(&r == &g && &ra == &arr[1] && pf == &fn);
static_assert(pmb == &S::b && pmf == &S::f && pmn == nullptr && pdx == &D::x);
static_assert(h.p == &arr[3] && X<&g>::self == &g && XR<g>::self == &g);
static_assert(HW<h>::obj == HW<Holder{&arr[3], 2}>::obj && HW<h>::v == 14);

int main() {
  int same = ax == &X<&g>::x && axa == &X<&arr[2]>::x && axr == &XR<g>::x &&
             axm == &XM<&S::b>::x && ah == &HW<h>::x && ax != axa &&
             pg == &g && &r == &g && &ra == &arr[1] && pend == arr + 4;
  printf("%d %d %d %d %d %d %d\n", same, *pa, s.*pmb, d.*pdx, *HW<h>::obj->p, HW<h>::v,
         (s.*pmf)() == 7);
  return same ? 0 : 1;
}
