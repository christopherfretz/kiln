// [module.unit]/3-8, [module.import]/7, [basic.lookup.argdep]/4,
// [module.reach]/5, spec/modules.md §11.11 (stage S6): a primary
// interface re-exporting an interface partition and importing an
// implementation partition, a module importing one module and
// re-exporting another, an implementation unit and a consumer. Under
// `--bmi-image` every unit's image follows its replayed module and
// import directives, and the program behaves exactly as with the
// replayed sources: ADL finds pa::norm through the re-exported
// partition, and top::total sees both partitions' module-linkage names.
//@ expect: run
//@ std: c++20
//@ module: imgu=Inputs/img-using.cppm
//@ module: imgp:a=Inputs/img-part-a.cppm
//@ module: imgp:b=Inputs/img-part-b.cppm
//@ module: imgp=Inputs/img-part.cppm
//@ module: imgr=Inputs/img-part-reexp.cppm
//@ unit: Inputs/img-part-impl.cpp
//@ stdout: 25 42 9 7 14
import imgr;
extern "C" int printf(const char *, ...);
static_assert(norm(pa::Pt{3, 4}) == 25 && top::answer() == 42);
static_assert(unwrap(top::Wrap{9}) == 9 && rr::seven() == 7);
int main() {
  pa::Pt p{3, 4};
  pa::Coord c = 7;
  printf("%d %d %d %d %d\n", norm(p), top::answer(), unwrap(top::Wrap{9}), c, top::total());
}
