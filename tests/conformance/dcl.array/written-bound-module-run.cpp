// [dcl.array]/1, [temp.spec.partial.match], [temp.expl.spec] across a
// module: bounds and partial or variable specialization arguments written
// as calls in the interface keep their values in the importer
// (spec/types.md 7.2 "Written bounds"; a module image carries a
// non-dependent written bound as its value). Reduced from real code.
//@ module: written_bounds_mod=Inputs/written_bounds_mod.cppm
//@ std: c++26
//@ expect: run
//@ stdout: 2 5 1 0 3 4
import written_bounds_mod;
extern "C" int printf(const char *, ...);

static_assert(__is_same(decltype(arr), int[2]));

int main() {
  TA<int> ta{};
  printf("%d %d %d %d %d %d\n", by_ref(arr),
         static_cast<int>(sizeof(ta.a) / sizeof(int)), P<char, 1>::k,
         P<int, 3>::k, v<char, 3>, v<int, 4>);
}
