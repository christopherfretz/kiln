// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5,
// [module.interface], spec/modules.md §11.11 "Part 2a": a default
// argument's temporaries used in a for-range-initializer persist until the
// end of the loop, also in an inline, constexpr or template body that an
// importer takes from a module. Sema extends them through a copy of the
// default argument rebuilt for the use (`default_arg_rewritten`,
// spec/sema.md §7.6a); under `--bmi-image` the importer builds these
// bodies from the declaration image, which must carry that copy, or the
// temporaries would die at the end of the initializer (`-7 1 2`).
// WP-1166.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ module: imgdt=Inputs/img-default-temps.cppm
//@ module: imgdtt=Inputs/img-default-temps-tpl.cppm
//@ stdout: 1 2 -7 0 -8 8 0 11 12 -9 0 | 5 | end
import imgdt;
import imgdtt;
extern "C" int printf(const char*, ...);
static_assert(dt::sum() == 5);

int main(int argc, char**) {
  dt::loop();
  dt::tloop(argc);
  for (int i = 0; i < dt::ntrace; ++i)
    printf("%d ", dt::trace[i]);
  printf("| %d | end\n", dt::sum());
}
