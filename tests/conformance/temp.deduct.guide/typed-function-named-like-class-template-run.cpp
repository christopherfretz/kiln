// [temp.deduct.guide]/1, [dcl.fct]/2: a declarator-id that names a class
// template followed by `( ... ) ->` is a deduction guide only when the
// decl-specifier-seq has no type specifier. With `auto` it declares a
// function with a trailing return type, here a member function template
// named like a namespace-scope class template (as a sender library's
// `__manual_lifetime::__construct_from` is, next to its
// `__construct_from` function object; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 7 8 9
extern "C" int printf(const char *, ...);

namespace n {
template <class T> struct make { int operator()() const { return 1; } };
template <class T> struct get {};

template <class T> class holder {
public:
  template <class F> constexpr auto make(F &&f) -> T & {
    buf_ = static_cast<T>(f());
    return buf_;
  }
  constexpr auto get() const -> T { return buf_; }

private:
  T buf_{};
};

template <class T> struct box {};
} // namespace n

namespace m {
using namespace n;
auto box() -> int { return 9; }
} // namespace m

int main() {
  n::holder<int> h;
  int a = h.make([] { return 7; });
  n::holder<long> l;
  l.make([] { return 8L; });
  printf("%d %ld %d\n", a, l.get(), m::box());
}
