// [expr.ref]/7, [class.temporary]/2, [expr.const]: libc++-shaped (reduced
// from span/at.pass.cpp): a member function of a class template prvalue
// returned by a function, called in a constant expression, reading a member
// of another prvalue; the out-of-range path throws, which is not constant.
// Pinned to C++23: in C++26 a caught or uncaught throw is evaluated (P3068).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:7 note "subexpression not valid in a constant expression"
//@ diag: 36:10 note "in call to 's.subspan(1).at(4)'"
//@ diag: 41:15 note "in call to 'test_at(4)'"
using size_t = decltype(sizeof 0);
struct out_of_range { const char *what; };
template <class T, size_t Extent = size_t(-1)>
class span {
  T *data_;
  size_t size_;
 public:
  constexpr span(T *d, size_t n) : data_(d), size_(n) {}
  template <size_t N>
  constexpr span(T (&a)[N]) : data_(a), size_(N) {}
  constexpr size_t size() const { return size_; }
  constexpr T &at(size_t i) const {
    if (i >= size_)
      throw out_of_range{"span"};
    return data_[i];
  }
  constexpr span subspan(size_t off) const { return span(data_ + off, size_ - off); }
  constexpr T *data() const { return data_; }
};
template <class T, size_t N> span(T (&)[N]) -> span<T, N>;
constexpr int test_at(size_t i) {
  int arr[] = {1, 2, 3, 4};
  span<int> s(arr, 4);
  return s.subspan(1).at(i) + span(arr).size() + *s.subspan(2).data();
}
static_assert(test_at(0) == 2 + 4 + 3);
static_assert(test_at(2) == 4 + 4 + 3);
static_assert(span<const int, 2>(nullptr, 2).size() == 2);
static_assert(test_at(4) == 0);
