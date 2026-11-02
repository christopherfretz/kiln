// [stmt.ranged]/1.3, [temp.constr.atomic]/3, [temp.deduct.general]/8: while
// the range-for builds `__range.begin()`, overload resolution checks the
// constraints of a `begin() const requires C<const V>` candidate; an
// invalid expression in that check (here a call to a deleted function, as
// with libc++'s `ranges::swap` poison pill in join_view) makes the
// constraint unsatisfied and is not a hard error. Regression for libc++
// range.join/general.pass.cpp. Matches clang 19.
//@ expect: run
namespace lib {
template <class T> void swap(T&, T&) = delete;
template <class T>
concept swappable = requires(T& a) { swap(a, a); };
}  // namespace lib

struct It {
  int* p;
  int& operator*() const { return *p; }
  It& operator++() { ++p; return *this; }
  bool operator!=(const It& o) const { return p != o.p; }
};
struct Inner { int* b; int* e; };

template <class V>
struct View {
  V v;
  It begin() { return {v.b}; }
  It begin() const requires lib::swappable<const V> { return {v.b}; }
  It end() { return {v.e}; }
  It end() const requires lib::swappable<const V> { return {v.e}; }
};

template <class R>
int sum(R& r) {
  int s = 0;
  for (int x : r) s += x;
  return s;
}

int main() {
  int a[3] = {1, 2, 3};
  View<Inner> v{{a, a + 3}};
  int s = 0;
  for (int x : v) s += x;
  return (s == 6 && sum(v) == 6) ? 0 : 1;
}
