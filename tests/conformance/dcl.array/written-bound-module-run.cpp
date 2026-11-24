// [dcl.array]/1, [dcl.fct]/5, [temp.spec.partial.match], [temp.expl.spec],
// [temp.spec.partial.member] across a module: bounds and specialization
// arguments written as calls in the interface keep their values in the
// importer, which never evaluates them again (spec/types.md 7.2 "Written
// bounds", spec/modules.md 11.11 "Arguments as written"): a class,
// variable or function template's explicit specialization, a partial
// specialization, a member template's partial specialization, an array
// parameter adjusted to a pointer. Reduced from real code.
//@ module: written_bounds_mod=Inputs/written_bounds_mod.cppm
//@ std: c++26
//@ expect: run
//@ stdout: 2 5 1 2 0 3 4 6 3 7 0
import written_bounds_mod;
extern "C" int printf(const char *, ...);

static_assert(__is_same(decltype(arr), int[2]));
static_assert(__is_same(decltype(par), int(int *)));

int main() {
  TA<int> ta{};
  int three[3] = {1, 2, 3};
  printf("%d %d %d %d %d %d %d", by_ref(arr),
         static_cast<int>(sizeof(ta.a) / sizeof(int)), P<char, 1>::k,
         P<int, 2>::k, P<int, 3>::k, v<char, 3>, v<int, 4>);
  printf(" %d %d %d %d\n", fh<6>(), par(three), O<long>::In<char, 7>::k,
         O<long>::In<char, 8>::k);
}
