// [module.interface], [class.virtual], [class.abstract], [class.dtor]/14,
// [class.mi]/4, [class.base.init]/7, [expr.const]/5, spec/modules.md
// §11.11 "Classes (S3)" (WP-1120): polymorphic classes exported from a
// module. The importer calls their virtual functions through base
// pointers and references, overrides a pure virtual function of an
// abstract base and a virtual destructor, deletes through a base pointer,
// walks a diamond of virtual bases, and constant-evaluates an inline-only
// polymorphic class; under `--bmi-image` every class is built from the
// declaration image, with the replay's behaviour.
//@ expect: run
//@ std: c++20
//@ module: imgp=Inputs/img-poly.cppm
//@ stdout: animal 0 0, dog 4 8, animal 2 4, puppy 4 8, 1211321 | 9 18 | 117 117 117 21 7 | 6 2
import imgp;
extern "C" int printf(const char *, ...);
using namespace ip;
static_assert(kRun == 10 && Counter().run(3) == 3 && Double().run(2) == 4);

struct Sq : Shape {
  int s;
  explicit Sq(int x) : s(x) {}
  int area() const override { return s * s; }
};
struct Puppy : Dog {
  ~Puppy() override { trace = trace * 10 + 3; }
  const char *name() const override { return "puppy"; }
};

int main(int argc, char **) {
  Animal *as[] = {new Animal, new Dog, new Bird, new Puppy};
  for (Animal *a : as)
    printf("%s %d %d, ", a->name(), a->legs(), a->twice());
  for (Animal *a : as)
    delete a;
  printf("%ld | ", trace);
  Sq q(argc + 2);
  const Shape &s = q;
  printf("%d %d | ", area_of(s), s.doubled());
  Join j;
  Left &l = j;
  Right &r = j;
  Base &b = j;
  printf("%d %d %d %d %d | ", j.id(), l.id(), b.id(), r.r(), j.b);
  Double d;
  Counter &c = d;
  printf("%d %d\n", c.run(argc + 2), c.step());
}
