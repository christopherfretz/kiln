// [class.mi]/4, [class.base.init]/13, [class.dtor]/13, [expr.new],
// [expr.delete], [expr.dynamic.cast] (C++26, P3533R2), spec/oven.md
// "Virtual bases": a deep diamond during constant evaluation. Top reaches a
// 64-deep chain of virtual bases (Chain<N> : virtual Chain<N-1>) through two
// paths, so its complete object holds 66 virtual base subobjects, each
// constructed once by Top's constructor and destroyed once by its
// destructor. 256 rounds allocate one, convert to virtual bases at both
// ends of the chain, call through them, dynamic_cast back and delete it
// through the root: the Oven's memory stays at one object (no growth per
// round), and the step count is linear in the rounds.
//@ skip(clang): neither clang 19 nor clang 22 implements P3533
//@ expect: compile-ok
//@ strict-diags
//@ oven: 46 steps<=40000 calls<=36000 allocs<=256 peak_bytes<=1072 peak_arena<=1536
struct Root {
  int r = 1;
  constexpr virtual int depth() const { return 0; }
  constexpr virtual ~Root() = default;
};
// A 64-deep chain of virtual bases: Chain<N> : virtual Chain<N-1>.
template <int N> struct Chain : virtual Chain<N - 1> {
  int v = N;
  constexpr int depth() const override { return N; }
};
template <> struct Chain<0> : virtual Root {};
constexpr int Depth = 64;
// Diamond tops sharing the whole chain.
struct L : virtual Chain<Depth> { int l = 2; };
struct R : virtual Chain<Depth> { int rr = 3; };
struct Top : L, R {
  int t = 4;
  constexpr int depth() const override { return 1000; }
};
constexpr long churn(int reps) {
  long sum = 0;
  for (int i = 0; i < reps; ++i) {
    Top* t = new Top;
    Root* r = static_cast<L*>(t);
    Chain<Depth / 2>* mid = static_cast<R*>(t);
    sum += r->depth() + mid->v + t->Chain<1>::v + t->l + t->rr;
    sum += dynamic_cast<Top*>(mid) == t;
    delete r;
  }
  return sum;
}
static_assert(churn(256) == 256L * (1000 + 32 + 1 + 2 + 3 + 1));
