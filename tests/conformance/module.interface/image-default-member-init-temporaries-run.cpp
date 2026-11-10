// [class.temporary]/7 (C++23, P2718R0), [class.temporary]/6 (CWG1815),
// [module.interface], spec/modules.md §11.11 "Part 2a": a default member
// initializer's temporaries used in a for-range-initializer persist until
// the end of the loop, and one a reference member binds persists with the
// variable, also in an inline or constexpr body that an importer takes
// from a module. Sema extends them through a copy of the member's
// initializer rebuilt for the use (`default_arg_rewritten`, spec/sema.md
// §7.6a); under `--bmi-image` the importer builds these bodies from the
// declaration image, which must carry that copy. WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
//@ module: imgdmi=Inputs/img-dmi-temps.cppm
//@ stdout: 1 2 -4 0 7 -7 0 | 124 | end
import imgdmi;
extern "C" int printf(const char*, ...);
static_assert(dm::cloop() == 124);

int main() {
  dm::loop();
  for (int i = 0; i < dm::ntrace; ++i)
    printf("%d ", dm::trace[i]);
  printf("| %d | end\n", dm::cloop());
}
