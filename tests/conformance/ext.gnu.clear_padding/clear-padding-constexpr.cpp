// __builtin_clear_padding in a constant expression (a kiln extension;
// GCC 16.1 rejects the call there): the Oven checks that the pointer
// designates storage this evaluation may modify and otherwise does
// nothing, since it does not model padding bits (they stay
// indeterminate). Value bits are unchanged. This keeps libstdc++'s
// `constexpr atomic(T)` usable in constant initialization for a T with
// padding: it clears T's padding under __has_builtin.
// __has_builtin is 1; __has_constexpr_builtin stays 0 (GCC's verdict).
//@ expect: compile-ok
//@ std: c++20
//@ skip(clang): clang has no __builtin_clear_padding (LLVM issue 64830)
//@ strict-diags
static_assert(__has_builtin(__builtin_clear_padding));
static_assert(!__has_constexpr_builtin(__builtin_clear_padding));

struct S { char c; int i; };
struct BF { unsigned a : 3; unsigned : 0; unsigned char b : 2; };

constexpr int f() {
  S s{1, 2};
  __builtin_clear_padding(&s);
  BF b{5, 3};
  __builtin_clear_padding(&b);
  S arr[3]{{1, 2}, {3, 4}, {5, 6}};
  __builtin_clear_padding(&arr);
  __builtin_clear_padding(&arr[1]);
  return s.c + s.i + b.a + b.b + arr[2].i;
}
static_assert(f() == 17);

// The shape of libstdc++'s atomic<T> constructor.
template <class T> struct atomic_like {
  T v;
  constexpr atomic_like(T x) : v(x) {
#if __has_builtin(__builtin_clear_padding)
    __builtin_clear_padding(__builtin_addressof(v));
#endif
  }
};
constinit atomic_like<S> g{S{7, 8}};
constexpr atomic_like<S> h{S{9, 10}};
static_assert(h.v.c == 9 && h.v.i == 10);

consteval bool heap() {
  S *p = new S{1, 2};
  __builtin_clear_padding(p);
  bool r = p->i == 2;
  delete p;
  return r;
}
static_assert(heap());
