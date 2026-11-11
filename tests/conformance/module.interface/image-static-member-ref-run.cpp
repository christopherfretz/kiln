// [temp.inst]/4, [basic.def.odr]/4, [module.interface], spec/modules.md
// "Templates as patterns" (WP-1300): an inline function body in the
// declaration image that names a constant static data member of an
// implicit class template specialization (`B<2>::v`, not odr-used) gets
// the member's initializer instantiated in the importer, as replay does;
// an odr-used one (`&B<7>::v`) is defined there too. Before the fix the importer left
// `B<2>::v` undefined and the link failed.
//@ expect: run
//@ std: c++20
//@ module: imgsm=Inputs/img-static-member-ref.cppm
//@ stdout: 2 34 7
import imgsm;
extern "C" int printf(const char *, ...);
int main() {
  printf("%d %d %d\n", g(), h(), *p());
  return 0;
}
