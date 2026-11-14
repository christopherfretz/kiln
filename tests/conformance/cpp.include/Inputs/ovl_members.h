// Forms a precompiled header's snapshot carries (WP-1368): overloaded member
// accesses in dependent calls, with their whole candidate sets.
#ifndef OVL_MEMBERS_H
#define OVL_MEMBERS_H
namespace om {
struct vec {
  int a[4];
  constexpr const int *begin() const { return a; }
  constexpr const int *end() const { return a + 4; }
};
// An implicit `this`: a plain overload and member templates.
struct dist_fn {
  constexpr long operator()(const int *f, const int *l, int) const { return l - f + 100; }
  template <class I> constexpr long operator()(I f, I l) const { return l - f; }
  template <class R> constexpr long operator()(R &&r) const { return operator()(r.begin(), r.end()); }
  template <class R> constexpr long twice(R &&r) const { return this->operator()(r) * 2; }
};
// `.` and `->` on a non-dependent object, a static candidate, explicit
// template arguments.
struct obj {
  constexpr int f(int) const { return 1; }
  constexpr int f(long) const { return 2; }
  static constexpr int f(double) { return 3; }
  template <class T> constexpr int g(T) const { return 10; }
  constexpr int g(int) const { return 20; }
};
template <class T> constexpr int dot(T t) { obj o; return o.f(t); }
template <class T> constexpr int arrow(T t) {
  obj o;
  const obj *p = &o;
  return p->f(t);
}
template <class T> constexpr int targs(T t) {
  obj o;
  return o.g<int>(t) + o.g(t);
}
// A member hidden by the derived class's; a using-declaration that brings
// the base's in.
struct base {
  constexpr int h(int) const { return 1; }
  constexpr int h(char) const { return 4; }
};
struct hides : base {
  constexpr int h(long) const { return 2; }
  constexpr int h(const char *) const { return 5; }
};
struct uses : base {
  using base::h;
  constexpr int h(long) const { return 3; }
  constexpr int h(double) const { return 6; }
};
template <class T> constexpr int hid(T t) { hides d; return d.h(t); }
template <class T> constexpr int used(T t) { uses d; return d.h(t); }
// A class template's `this->f`.
template <class V> struct gen {
  V v;
  template <class I> constexpr V run(I i) { return this->run(i, v); }
  template <class I> constexpr V run(I i, V w) { return i + w; }
  constexpr V run(int *, V) { return -1; }
};
// A private candidate.
struct priv {
  constexpr int p(long) const { return 1; }
private:
  constexpr int p(int) const { return 2; }
};
template <class T> constexpr int callp(T t) { priv x; return x.p(t); }
} // namespace om
#endif
