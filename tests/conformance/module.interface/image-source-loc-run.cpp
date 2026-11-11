// [support.srcloc.cons], [module.interface], spec/modules.md §11.11
// (WP-1310): the source-location builtins in an imported module's
// function bodies give the module's location and function, and in a
// default argument the call's; under `--bmi-image` the declaration image
// carries the builtin calls and the importer computes their values again.
//@ expect: run
//@ std: c++20
//@ module: imgsl=Inputs/img-source-loc.cppm
import imgsl;
int main() {
  const char *f = sl::func_here();
  if (sl::line_here() != 3 || f[0] != 'f' || f[9] != 0 || sl::tline<int>() != 5)
    return 1;
  if (sl::caller_line() != 14 || sl::use_line() != 7 || sl::member_line() != 11)
    return 2;
  return 0;
}
