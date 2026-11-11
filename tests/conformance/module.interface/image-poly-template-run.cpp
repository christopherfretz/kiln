// [class.virtual]/2, [class.abstract]/2, [temp.variadic]/5.2,
// [module.interface], spec/modules.md §11.11 "Polymorphic class templates
// and pack bases" (WP-1292): class templates with virtual functions
// (overriders, final, pure virtual, a defaulted virtual destructor) and a
// class template whose bases are a pack expansion, exported from a
// module, are instantiated by the importer and dispatch through their
// vtables; under `--bmi-image` they come from the declaration image and
// the program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgpoly=Inputs/img-polytempl.cppm
//@ stdout: 9 4 11 13 17 2 3
import imgpoly;
extern "C" int printf(const char *, ...);
static_assert(sizeof(pt::Many<pt::G1, pt::G2>) == 1);
int main() {
  pt::Sq<int> q;
  q.s = 3;
  const pt::Shape &s = q;
  pt::Impl<long> im;
  const pt::Abs<long> &a = im;
  pt::Many<pt::G1, pt::G2> m;
  printf("%d %d %d %d %d %d %d\n", s.area(), s.sides(), (int)a.f(), pt::tid<int>(s),
         pt::tid<long>(s), m.n(), m.one() + m.two());
  return 0;
}
