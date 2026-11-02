// [class.union.anon]/1, [temp.inst]: a block-scope anonymous union of a
// function template (and of a lambda or local class in one) declares its
// unnamed object and injects its members' names when the body is
// instantiated; unnamed bit-fields and unnamed members of a class
// template keep their layout.
//@ expect: run
//@ strict-diags
template<class T> constexpr T f() {
  struct L {
    struct { T a; } s;
    union { T b; char c; };
    constexpr T get() const { return s.a + b; }
  };
  L l{{1}, {2}};
  typedef struct { T v; } TD;
  TD td{3};
  union { struct { T p, q; }; long r; };
  p = 4;
  q = 5;
  auto lam = [&] { union { T m; int n; }; m = 6; return m; };
  auto gen = [](auto z) { union { decltype(z) m2; }; m2 = z; return m2; };
  return l.get() + td.v + p + q + lam() + gen(T(7));
}
static_assert(f<int>() == 28);
static_assert(f<long>() == 28);

template<class T> T g() {
  static union { T s; int t; };
  return ++s;
}

template<class T, int N> struct B {
  union { T a; char bytes[sizeof(T)]; };
  struct { T c; T d; };
  int : N;
  T e : N;
  int : 0;
  char f;
};
struct BI {
  union { int a; char bytes[sizeof(int)]; };
  struct { int c; int d; };
  int : 3;
  int e : 3;
  int : 0;
  char f;
};
static_assert(sizeof(B<int, 3>) == sizeof(BI));
static_assert(__builtin_offsetof(B<int, 3>, f) == __builtin_offsetof(BI, f));
constexpr B<int, 3> b{{1}, {2, 3}, 2, 4};
static_assert(b.a + b.c + b.d + b.e + b.f == 12);

int main() {
  if (f<int>() != 28)
    return 1;
  return g<int>() + g<int>() == 3 ? 0 : 2;
}
