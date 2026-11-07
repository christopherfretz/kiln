// [expr.mptr.oper]/6: a `.*` or `->*` whose result is a function can only
// be the operand of a function call. In the operand of `decltype` and
// `noexcept` the uncalled result is ill-formed, so forming it while
// substituting a function template's return type is a deduction failure
// ([temp.deduct.general]/8): an `invoke` overload taking `D T::*` drops out
// for a pointer to member function and the calling overload is chosen.
//@ expect: run
//@ std: c++20
//@ strict-diags
struct S {
  bool p() const { return true; }
  int v = 3;
};
struct invoke_fn {
  template <typename F, typename T, typename T1, typename... Args>
  constexpr auto operator()(F T::*f, T1&& t1, Args&&... args) const
      -> decltype((((T1&&)t1).*f)((Args&&)args...)) {
    return (((T1&&)t1).*f)((Args&&)args...);
  }
  template <typename D, typename T, typename T1>
  constexpr auto operator()(D T::*f, T1&& t1) const
      noexcept(noexcept(((T1&&)t1).*f)) -> decltype(((T1&&)t1).*f) {
    return ((T1&&)t1).*f;
  }
};
constexpr invoke_fn invoke{};
template <class F, class T>
constexpr bool can_noexcept(...) { return false; }
template <class F, class T, bool = noexcept(T{}.*F{})>
constexpr bool can_noexcept(int) { return true; }
static_assert(!can_noexcept<bool (S::*)() const, S>(0));
static_assert(can_noexcept<int S::*, S>(0));
int main() {
  S s;
  return invoke(&S::p, s) && invoke(&S::v, s) == 3 ? 0 : 1;
}
