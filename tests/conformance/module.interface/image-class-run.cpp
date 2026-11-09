// [module.interface], [class.mem], [class.derived], [class.mfct]/1,
// [class.ctor], [class.dtor], [class.temporary], spec/modules.md §11.11
// "Part 2a" (stage S3): classes exported from a module. The importer
// derives from them, constant-evaluates their members (static_assert)
// and runs them; under `--bmi-image` it builds every class, member and
// implicit special member from the declaration image, and the program
// behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imgc=Inputs/img-class.cppm
//@ stdout: 2 4 6 8 1 4 10 4 17 7 | sum 8 8 trace 126621 | created 2
import imgc;
extern "C" int printf(const char *, ...);
using namespace ic;
static_assert(kDiag == Point(4, 5) && kDiag.norm1() == 9 && Point::kDims == 2);
static_assert(kTotal == 4 + 10 + 6 + 5 && kTicks == 6 && kScore == 3);
static_assert(Both().total() == 1 + 4 + 3 + 2 && base2_of(Both(0, 9, 0)) == 9);
static_assert(Flags().sum() == 8 && sizeof(Flags) == 2 * sizeof(int));
static_assert(!Counter() && bool(Counter(Counter::Down).tick()));
static_assert(sizeof(Both) == 3 * sizeof(int) && sizeof(Word) == sizeof(unsigned));
static_assert(kOrigin == Point() && Word().u == 0);

// Cross-module inheritance.
struct Derived : Both {
  int extra = 7;
  constexpr int all() const { return total() + extra; }
};
static_assert(Derived().all() == 17 && Leaf().leaf() == 11);

int main(int argc, char **) {
  Point p(argc, argc + 1), q = p;
  q = q + p;
  Flags f;
  Counter c(argc > 5 ? Counter::Down : Counter::Up);
  c.tick().tick(Counter::Step{3});
  Both b(argc, 2, 3);
  const Base2 &r = b;
  Word w;
  w.u = 7;
  printf("%d %d %d %u %d %ld %d %d %d %u | ", q.x, q.y, q.norm1(), f.sum(), f.hits, c.get(),
         b.total(), r.two(), Derived().all(), w.u);
  printf("sum %d %d trace %ld | ", noisy_sum(argc), c.report(), trace);
  Point::created += 2;
  printf("created %d\n", Point::created);
}
