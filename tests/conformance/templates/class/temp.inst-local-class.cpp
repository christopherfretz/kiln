// Local classes and enumerations of templated functions ([temp.inst]/3,
// [class.local]): instantiated with their enclosing function, members
// referring to its template parameters. libc++ hash.h (__murmur2_or_cityhash
// unions with an anonymous struct) needed this.
//@ expect: compile-ok
//@ strict-diags
typedef unsigned long size_t;

template <class T> size_t bits(T v) {
  union {
    T t;
    struct {
      size_t a;
      size_t b;
    } s;
  } u;
  u.s.a = 0;
  u.s.b = 0;
  u.t = v;
  return u.s.a ^ u.s.b;
}

template <class T, int N> constexpr int local() {
  struct L {
    T x;
    struct Inner {
      static constexpr int k() { return N * 10; }
    };
    constexpr L(T v) : x(v + N) {}
    constexpr T get() const { return x * N; }
    static constexpr int n() { return N + sizeof(T); }
  };
  enum E { e0 = N, e1 };
  L l(T(1));
  int r = l.get() + L::n() + e1 + L::Inner::k();
  {
    struct M {
      constexpr int f() const { return 1; }
    };
    r += M().f();
  }
  {
    struct M {
      constexpr int f() const { return 2; }
    };
    r += M().f();
  }
  return r;
}
static_assert(local<int, 2>() == 6 + 6 + 3 + 20 + 3);
static_assert(local<char, 3>() == 12 + 4 + 4 + 30 + 3);

template <class T> struct C {
  constexpr int f(int k) const {
    struct D {
      int m;
      constexpr int g(int q) const { return m + q + int(sizeof(T)); }
    };
    D d{k};
    return d.g(1);
  }
};
static_assert(C<short>().f(4) == 7);

template <class T> auto make() {
  struct R {
    T v;
  };
  return R{T(5)};
}
static_assert(sizeof(make<long>()) == sizeof(long));

int use() { return int(bits(1.5) != 0) + int(bits((long double)2.0) != 0); }
