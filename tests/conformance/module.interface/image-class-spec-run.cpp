// [temp.spec.partial], [temp.expl.spec], [temp.spec.partial.match],
// [module.interface], spec/modules.md §11.11 "Templates as patterns"
// (WP-1283): partial and explicit specializations of class templates
// exported from a module (partial ones over a non-type parameter and
// over a pack, a declared but undefined explicit one, specializations in
// a reopened namespace and declared by a qualified name) are matched and
// instantiated by the importer;
// under `--bmi-image` they come from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgcs=Inputs/img-cspec.cppm
//@ stdout: 9 40 5 3 2 0 8
import imgcs;
extern "C" int printf(const char *, ...);
static_assert(cs::X<int *>::v == 2 && cs::X<int>::v == 4 && cs::X<long>::v == 1);
static_assert(cs::X<const int>::v == 3 && cs::X<const long>::v == 3);
static_assert(cs::A<char, 3>::v == 30 && cs::A<int, 0>::v == -1 && cs::A<long, 7>::v == 77);
static_assert(cs::A<long, 6>::v == 6);
static_assert(cs::L<int, char, short, long>::v == 102 && cs::L<>::v == -5 && cs::L<char>::v == 1);
static_assert(__is_same(cs::B<short &>::type, short));
static_assert(cs::F<int>::v == 0 && cs::F<int *>::v == 8);
static_assert(cs::X<int &>::v == 5 && cs::X<char>::v == 6 && cs::A<short, 1>::v == 11 && cs::A<int, 2>::v == 22);
int main() {
  int k = 9;
  cs::X<int *> a{&k};
  printf("%d %d %d %d %d %d %d\n", a.self().get(), cs::X<int>{}.f(), cs::used, cs::g(),
         cs::X<const int *>::v, (int)(sizeof(cs::E<long *>) / sizeof(long *)) - 2, cs::F<char *>::v);
  return 0;
}
