// [over.match.ctor]/1, [class.conv.fct]/1, [over.ics.user]/4: a conversion
// function never converts an object to its own class or to a base of it,
// so it is a non-viable candidate of such an initialization (clang
// `ovl_fail_trivial_conversion`) and the copy or move constructor is
// chosen: a conversion to `It<const T>` that is the class itself for
// `T = const int` is not ambiguous with the copy constructor; a deleted
// rvalue `operator view() &&` naming the class itself is not chosen for a
// prvalue argument; an explicit conversion function of a derived class to
// its base does not take part in the function-style cast `Less(c)`.
// -Wclass-conversion is not reported for a member of an instantiated class
// (clang suppresses it in instantiations). Matches clang 19.
//@ expect: run
//@ strict-diags
//@ diag: 49:12 warning "conversion function converting 'checked' to its base class 'Less' will never be used"
//@ stdout: 2 5 1 1 1
#include <cassert>
#include <functional>
extern "C" int printf(const char*, ...);

template <class T> struct It {
  T* p;
  constexpr It(T* q) : p(q) {}
  constexpr operator It<const T>() const noexcept { return {p}; }
  It next() const {
    It r = *this;
    ++r.p;
    return r;
  }
};

template <class P> struct heap {
  using view = heap<const P>;
  int v = 0;
  heap() = default;
  heap(const heap&) = default;
  operator view() const& noexcept { return {}; }
  operator view() && = delete;
};
heap<const int> mk() {
  heap<const int> h;
  h.v = 5;
  return h;
}
int take(heap<const int> h) { return h.v; }

struct Less { bool operator()(int a, int b) const { return a < b; } };
struct checked : Less {
  explicit checked(Less c) : Less(c) {}
  explicit operator Less() const { return *this; }
};

template <class Compare> struct checked_t : Compare {
  explicit checked_t(Compare c) : Compare(c) {}
  explicit operator Compare() const { return *this; }
};

int main() {
  const int a[2] = {1, 2};
  It<const int> i(a);
  int x = *i.next().p;
  int y = take(mk());
  checked c{Less{}};
  Less l = Less(c);
  checked_t<std::less<int>> ct{std::less<int>{}};
  std::less<int> sl = std::less<int>(ct);
  assert(std::less<int>(ct)(1, 2));
  printf("%d %d %d %d %d\n", x, y, l(1, 2), sl(1, 2), static_cast<Less>(c)(1, 2));
  return 0;
}
