// [expr.ref]/4, [class.mem.general]/8: the rejected side of
// member-access-in-deduction-from-class-being-instantiated.cpp. Outside a
// complete-class context (a member alias) the class being instantiated is
// incomplete: the access through `declval<C const&>()` is an error, as in
// clang.
//@ args: -std=c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:41 error "member access into incomplete type 'const C<int>'"
//@ diag: 21:8 note "in instantiation of template class 'C<int>' requested here"
//@ diag: 15:8 note "definition of 'C<int>' is not complete until the closing '}'"
template <class T> T&& declval();

template <class T>
struct C {
  int a_;
  template <class U>
  auto get(U) const -> decltype(a_) { return a_; }
  using R = decltype(declval<C const&>().get(0));
};
C<int> c;
