// [expr.unary.op]/3: `&C::m` names a pointer to member even where `this`
// may be used and `C` is the current instantiation of a class template
// whose member `m` has a dependent type. Instantiation rebuilds `&C::m`,
// never the implicit member access `&this->m` (in a member function's
// body or trailing return type, or a friend's trailing return type).
//@ expect: run
//@ std: c++20
//@ strict-diags
template <class T> struct R {
  T rng_;
  int body() const { T R::*m = &R::rng_; return this->*m; }
  auto ret() const -> decltype(&R::rng_) { return &R::rng_; }
  auto use(R const& p) const -> decltype(p.*&R::rng_) { return p.*&R::rng_; }
  template <int N>
  friend constexpr auto get(R const& p) -> decltype(p.*&R::rng_) {
    return p.*&R::rng_;
  }
  friend constexpr auto plus1(R const& p) -> decltype(R::rng_ + 1) {
    return p.rng_ + 1;
  }
};
static_assert(get<0>(R<int>{4}) == 4);
static_assert(plus1(R<long>{4}) == 5);
int main() {
  R<int> r{3};
  int R<int>::*m = r.ret();
  return r.body() == 3 && r.*m == 3 && r.use(r) == 3 && get<1>(r) == 3 ? 0 : 1;
}
