// [temp.spec.partial.match]/2, [temp.spec.partial.general]/9,
// [temp.mem]/1, [over.literal]/5, [temp.mem.func]/1, [module.interface],
// spec/modules.md §11.11 "Forms carried by WP-1358": specializations of
// constrained and declared-before-defined partial specializations, a
// dependent partial specialization argument, variable template partial
// specializations, member function template specializations, literal
// operator template arguments, a trailing requires-clause of a member
// and a constrained member template, used by an importer; under `--bmi-image` they come from the
// declaration image and the program behaves exactly as with the
// replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgsf=Inputs/img-spec-forms.cppm
//@ stdout: 3210 7200 4321 3
import imgsf;
extern "C" int printf(const char *, ...);
using namespace sf;
static_assert(A<double *>::k == 2 && A<char *>::k == 3 && A<int>::k == 1);
static_assert(has<int>::value && sizeof(cp) == 1 && traits<int *>::k == 2);
static_assert(ks == 239 && k_v<double *> == 3);
int main() {
  printf("%d %ld %ld %d\n", a_sum(), in_mins(hours(2)), secs(), w_use());
  return (12_s).n == 12 && W<long>{}.g() == 2 && cm_use() == 0 && CM<char>(1, 0).k == 2 ? 0 : 1;
}
