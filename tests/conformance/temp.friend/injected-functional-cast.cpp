//@ expect: run
//@ stdout: 7 6 1
// [temp.local]/1: a class template's injected-class-name used without
// arguments as a functional cast, `X(args)` and `X{args}`, in a class
// that also befriends every specialization of itself (libc++'s
// `reverse_iterator::operator+`). WP-121.
extern "C" int printf(const char *, ...);
template <class I> class Rev {
  template <class U> friend class Rev;
  I cur;

public:
  constexpr Rev(I c) : cur(c) {}
  constexpr Rev operator+(int n) const { return Rev(cur + n); }
  constexpr Rev operator-(int n) const { return Rev{cur - n}; }
  template <class U> constexpr bool same(const Rev<U> &o) const { return cur == I(o.cur); }
  constexpr I get() const { return cur; }
};
static_assert((Rev<int>(1) + 2).get() == 3);
int main() {
  Rev<int> r = (Rev<int>(4) + 5) - 2;
  Rev<long> l(6);
  printf("%d %ld %d\n", r.get(), l.get(), (int)(r - 1).same(l));
}
