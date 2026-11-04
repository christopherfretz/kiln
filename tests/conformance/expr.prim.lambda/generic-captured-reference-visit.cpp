// [expr.prim.lambda.capture]/8, [except.spec]/13: the shape of a
// header-only configuration library's node comparison. A visitor whose
// `noexcept` and return type are probed through `declval` (instantiating
// the generic call operator in unevaluated operands first) is called once
// per alternative of a two-alternative switch; the lambda captures a
// reference local by `[&]` and uses it as the operand of `&` under a
// `reinterpret_cast` to its parameter's type. Each specialization is
// emitted against the closure's reference member (WP-581).
//@ expect: compile-ok
//@ std: c++20
//@ args: -O2
template <class T> T&& declval() noexcept;
template <class T> struct remove_ref { using type = T; };
template <class T> struct remove_ref<T&> { using type = T; };
template <class T> using remove_ref_t = typename remove_ref<T>::type;

enum class kind { integer, string };
struct node {
  kind k;
  kind type() const noexcept { return k; }
  template <class T> const T& ref_cast() const& noexcept { return *static_cast<const T*>(this); }
  template <class F, class N>
  static constexpr bool nothrow_with =
      noexcept(declval<F>()(declval<N>().template ref_cast<struct integer>()));
  template <class F>
  decltype(auto) visit(F&& f) const& noexcept(nothrow_with<F&&, const node&>);
};
struct integer : node { long v; };
struct string : node { const char* s; };
inline bool operator==(const integer& a, const integer& b) noexcept { return a.v == b.v; }
inline bool operator==(const string& a, const string& b) noexcept { return a.s == b.s; }

template <class F>
decltype(auto) node::visit(F&& f) const& noexcept(nothrow_with<F&&, const node&>) {
  switch (type()) {
  case kind::integer: return static_cast<F&&>(f)(ref_cast<integer>());
  case kind::string: return static_cast<F&&>(f)(ref_cast<string>());
  }
  return decltype(static_cast<F&&>(f)(ref_cast<integer>())){};
}

bool equal(const node* const* l, const node* const* r, int n) noexcept {
  for (int i = 0; i < n; ++i) {
    const auto lhs_type = l[i]->type();
    const node& rhs_ = *r[i];
    const auto rhs_type = rhs_.type();
    if (lhs_type != rhs_type) return false;
    const bool eq = l[i]->visit([&](const auto& lhs_) noexcept {
      return lhs_ == *reinterpret_cast<remove_ref_t<decltype(lhs_)>*>(&rhs_);
    });
    if (!eq) return false;
  }
  return true;
}
