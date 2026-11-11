// [temp.dep.candidate], [temp.res.general]/2, [basic.lookup.argdep]/4,
// [module.interface], spec/modules.md §11.11 "Templates as patterns"
// (WP-1279): patterns calling unresolved names (overload sets, qualified
// names, explicit template arguments, variable templates, operators,
// argument-dependent lookup reaching the importer's own functions at the
// point of instantiation, in a fold) exported from a module and
// instantiated by the importer; under `--bmi-image` every template comes
// from the declaration image and the program behaves exactly as with
// the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgu=Inputs/img-lookup.cppm
//@ stdout: 2 3 6 8 3 5 6 70 5 10
import imgu;
extern "C" int printf(const char *, ...);
struct U { int w; };
constexpr int h(U u) { return u.w * 10; }
static_assert(iu::call(1) == 2 && iu::next<4>() == 5 && iu::sz<char>() == 4);
int main() {
  printf("%d %ld %d %d %d %d %d %d %d %d\n", iu::call(1), iu::call(1L), iu::qual(5), iu::expl(4),
         iu::next<2>(), iu::sz<short>(), iu::adl(iu::S{6}), iu::adl(U{7}),
         iu::plus(iu::S{2}, iu::S{3}).v, iu::sum(1, 2L, 3));
  return 0;
}
