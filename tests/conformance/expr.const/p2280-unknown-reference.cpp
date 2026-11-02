// [expr.const]/5.10, /10 (C++23, P2280R4, a DR): an id-expression naming a
// reference whose referent is unknown is usable in a constant
// expression as long as the referent's value is not needed: binding it to
// another reference, passing it on, and calling a non-virtual member
// function that does not read the object, including from templates.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
template <class T, size_t N> constexpr size_t forward_size(T (&a)[N]) { return size(a); }

struct Fixed {
  int data[3];
  constexpr size_t capacity() const { return 3; }
  static constexpr size_t limit() { return 8; }
};

void arrays(int (&a)[4], const char (&s)[6]) {
  static_assert(size(a) == 4);
  static_assert(size(s) == 6);
  static_assert(forward_size(a) == 4);
}

void members(Fixed &f, const Fixed &cf) {
  static_assert(f.capacity() == 3);
  static_assert(cf.capacity() == 3);
  static_assert(f.limit() == 8);
  static_assert(size(f.data) == 3);
}

template <class C> constexpr bool check(C &c) {
  static_assert(c.capacity() == 3);
  return true;
}
template bool check<Fixed>(Fixed &);

template <class T, size_t N> void in_template(T (&a)[N]) {
  static_assert(size(a) == N);
}
template void in_template<long, 7>(long (&)[7]);

