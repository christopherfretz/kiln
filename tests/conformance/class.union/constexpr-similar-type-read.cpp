// [class.union.general]/2, /6, [basic.lval]/11, [dcl.init.ref]/5.1
// (CWG2352), [expr.const]/10.9: a glvalue of a type similar to the
// object's designates that object, so a `const int *const &` bound
// directly to an `int *` member of a union's active member (or a
// `const int *const (&)[2]` bound to an `int *[2]`) reads the active
// member, however the member became active: std::construct_at
// (placement new), a mem-initializer, a default member initializer, or an
// assignment of the form [class.union.general]/6 covers (through a
// nested member access and an array element too). The Oven compares the
// object it reaches and the member recorded active by one identity
// (Types::same_object; spec/oven.md "Unions"). Reduced from real code:
// a fill-insert into a vector with spare capacity copies its value out
// of a temporary held in a union (`_Temporary_value`), and the copy
// assignment of an element owning an allocation reads the source's
// pointers through `const T *const &`.
//@ expect: compile-ok
//@ strict-diags
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class... A> constexpr T *construct_at(T *p, A &&...a) {
  return ::new ((void *)p) T(static_cast<A &&>(a)...);
}
template <class T> constexpr void destroy_at(T *p) { p->~T(); }
} // namespace std
namespace ns {
template <class T> constexpr T &&move(T &t) { return static_cast<T &&>(t); }
// Reads `p` as a `const T *` glvalue bound directly to it.
template <class T> constexpr T peek(const T *const &p) { return *p; }
// Compares `p`, read as a `const T *` glvalue, with `q`.
template <class T> constexpr bool is(const T *const &p, const T *q) { return p == q; }
} // namespace ns

struct Row { long n; int *p; };

// std::construct_at.
constexpr int by_construct_at() {
  int x = 4;
  union U { constexpr U() : c() {} char c; Row r; } u;
  std::construct_at(&u.r, Row{1, &x});
  return ns::peek(u.r.p);
}
static_assert(by_construct_at() == 4);

// A mem-initializer.
struct ByInit {
  union { char c; Row r; };
  constexpr ByInit(int *q) : r{2, q} {}
};
constexpr int by_mem_initializer() {
  int x = 6;
  ByInit b(&x);
  return ns::peek(b.r.p);
}
static_assert(by_mem_initializer() == 6);

// A default member initializer.
int g;
struct ByDefault {
  union { char c; int *p = &g; };
};
constexpr bool by_default_member_initializer() {
  ByDefault b;
  return ns::is(b.p, &g);
}
static_assert(by_default_member_initializer());

// Assignment: `u.r.p = q` makes `r` active ([class.union.general]/6),
// through a nested member access and through an array element.
constexpr int by_nested_assignment() {
  int x = 8;
  union U { char c; Row r; } u{.c = 0};
  u.r.p = &x;
  return ns::peek(u.r.p);
}
static_assert(by_nested_assignment() == 8);

constexpr int by_array_element_assignment() {
  int x[2] = {9, 10};
  union V { char c; int *a[2]; } v{.c = 0};
  v.a[1] = &x[1];
  v.a[0] = &x[0];
  const int *const (&ra)[2] = v.a;
  return *ra[0] + *ra[1];
}
static_assert(by_array_element_assignment() == 19);

// Through a base-class path.
struct Data { int *p; };
struct Impl : Data { long n; };
constexpr int through_base() {
  int x = 11;
  union U { constexpr U() : c() {} char c; Impl i; } u;
  std::construct_at(&u.i, Impl{{&x}, 1});
  const Data &d = u.i;
  return ns::peek(d.p);
}
static_assert(through_base() == 11);

// Through a nested anonymous union.
struct Nested {
  union {
    char c;
    union { long l; int *p; };
  };
};
constexpr int through_nested_anonymous_union() {
  int x = 12;
  Nested n{};
  n.p = &x;
  return ns::peek(n.p);
}
static_assert(through_nested_anonymous_union() == 12);

// A union inside an array element.
constexpr int in_array_element() {
  int x = 13;
  union U { char c; Row r; } us[3] = {{.c = 0}, {.c = 0}, {.c = 0}};
  us[2].r = Row{1, &x};
  return ns::peek(us[2].r.p);
}
static_assert(in_array_element() == 13);

// A union member of class type with its own union inside.
struct Inner { union { char c; int *p; }; };
constexpr int union_in_union_member() {
  int x = 14;
  union U { constexpr U() : c() {} char c; Inner in; } u;
  std::construct_at(&u.in);
  u.in.p = &x;
  return ns::peek(u.in.p);
}
static_assert(union_in_union_member() == 14);

// The copy and the move assignment of an object held in a union member
// read its pointers through `const T *const &` (the reduced fill-insert).
struct Owner {
  int *p = nullptr;
  constexpr Owner() = default;
  constexpr explicit Owner(int v) : p(new int(v)) {}
  constexpr Owner(const Owner &o) : p(new int(ns::peek(o.p))) {}
  constexpr Owner(Owner &&o) : p(o.p) { o.p = nullptr; }
  constexpr Owner &operator=(const Owner &o) {
    const int *const &src = o.p;
    int *q = new int(*src);
    delete p;
    p = q;
    return *this;
  }
  constexpr Owner &operator=(Owner &&o) {
    int *const &src = o.p;
    delete p;
    p = src;
    o.p = nullptr;
    return *this;
  }
  constexpr ~Owner() { delete p; }
};
struct Elem { long name; Owner list; };
template <class T> struct Temporary {
  union Storage { constexpr Storage() : b() {} constexpr ~Storage() {} char b; T val; } s;
  template <class... A> constexpr Temporary(A &&...a) { std::construct_at(&s.val, static_cast<A &&>(a)...); }
  constexpr T &get() { return s.val; }
  constexpr ~Temporary() { std::destroy_at(&s.val); }
};
template <class T> constexpr void fill(T *first, T *last, const T &v) {
  for (; first != last; ++first) *first = v;
}
constexpr int fill_from_temporary() {
  Elem es[4] = {{1, Owner(1)}, {2, Owner(2)}, {3, Owner(3)}, {4, Owner(4)}};
  Temporary<Elem> tmp(Elem{9, Owner(7)});
  fill(es + 1, es + 3, tmp.get());
  return *es[0].list.p + *es[1].list.p + *es[2].list.p + *es[3].list.p;
}
static_assert(fill_from_temporary() == 1 + 7 + 7 + 4);

constexpr int move_out_of_temporary() {
  Elem e{1, Owner(1)};
  Temporary<Elem> tmp(Elem{2, Owner(5)});
  e = ns::move(tmp.get());
  return *e.list.p + (tmp.get().list.p == nullptr);
}
static_assert(move_out_of_temporary() == 6);

// The member named through the instantiated class and through a
// dependent path.
template <class T> struct Holder {
  union { char c; T v; };
  constexpr Holder(const T &x) : v(x) {}
  constexpr auto read() const { return ns::peek(v.p); }
};
template <class H> constexpr int dependent_read(const H &h) { return ns::peek(h.v.p); }
constexpr int through_template() {
  int x = 15;
  Holder<Row> h(Row{1, &x});
  return h.read() + dependent_read(h) + ns::peek(h.Holder<Row>::v.p);
}
static_assert(through_template() == 45);
