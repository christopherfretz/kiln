// Constant emission of array initializers (WP-1008, spec/codegen.md
// "Constant emission of initializers"): clang `AggExprEmitter::
// EmitArrayInit` copies an array of trivially copyable elements whose
// explicit elements take more than 16 bytes from a private `constinit`
// global when `ConstantEmitter::tryEmitForInitializer` can emit the list,
// folding each element it cannot spell directly (a constexpr call, a
// constexpr constructor) as a trial rvalue ([dcl.init.aggr]/4,
// [dcl.init.list]/5). A local that is a POD aggregate takes clang's
// `IsConstantAggregate` path instead (`__const.<fn>.<var>` or stores).
namespace std {
template <class E> class initializer_list {
  const E* b;
  unsigned long n;
  constexpr initializer_list(const E* b, unsigned long n) : b(b), n(n) {}

public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr const E* begin() const { return b; }
  constexpr unsigned long size() const { return n; }
};
} // namespace std

struct tm_ {
  int sec, min, hour, mday, mon, year;
  long off;
  const char* zone;
};
constexpr tm_ make_tm(int year, int mon, int mday) {
  auto t = tm_();
  t.mday = mday;
  t.mon = mon - 1;
  t.year = year - 1900;
  return t;
}
void use(const void*);
void use_list(std::initializer_list<tm_>);

// The backing array of an initializer list ([dcl.init.list]/5).
void list_of_calls() { use_list({make_tm(1975, 12, 29), make_tm(2000, 1, 1)}); }

// A POD local: `IsConstantAggregate`, no `constinit`.
void pod_local() {
  tm_ a[2] = {make_tm(1975, 12, 29), make_tm(2000, 1, 1)};
  use(a);
}

// Constexpr constructors of a trivially copyable class that is not POD.
struct S {
  int x;
  constexpr S(int v = 7) : x(v) {}
};
void ctor_elements() {
  S s[5] = {1, 2, 3, 4, 5};
  use(s);
}

// 16 bytes of explicit elements: element-wise.
void small_list() {
  S s[4] = {1, 2, 3, 4};
  use(s);
}

// An array member of an aggregate that is not constant.
void member_array(int y) {
  struct {
    int a[5];
    int b;
  } t = {{1, 2, 3, 4, 5}, y};
  use(&t);
}

// A filler: the whole array is copied, the explicit elements decide.
void with_filler(int y) {
  struct {
    S a[8];
    int b;
  } t = {{1, 2, 3, 4, 5}, y};
  use(&t);
}

// Not constant: an element reads a parameter, or has a side effect.
int g;
void not_constant(int y) {
  S a[5] = {1, 2, 3, 4, y};
  use(a);
  S b[5] = {1, 2, 3, 4, g++};
  use(b);
}

// Not trivially copyable: the constructors run.
struct N {
  int x;
  constexpr N(int v) : x(v) {}
  N(const N&);
};
void not_trivially_copyable() {
  N n[5] = {1, 2, 3, 4, 5};
  use(n);
}

// Nested arrays: the outer list is the constant, the inner ones are not
// emitted.
void nested() {
  S m[2][3] = {{1, 2, 3}, {4, 5, 6}};
  use(m);
}

// A trial fold: `__builtin_is_constant_evaluated()` is false.
constexpr int ice() { return __builtin_is_constant_evaluated() ? 1 : 2; }
void trial() {
  S s[5] = {ice(), ice(), 3, 4, 5};
  use(s);
}

// A constructor's member initializer.
struct M {
  S a[6];
  M() : a{1, 2, 3, 4, 5, 6} {}
};
void member_init() {
  M m;
  use(&m);
}

// A temporary array bound to a reference.
using A5 = S[5];
void temporary() {
  const A5& r = A5{1, 2, 3, 4, 5};
  use(r);
}
