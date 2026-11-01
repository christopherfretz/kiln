// [temp.explicit]/4,6, [except.spec]/4: an explicit instantiation of a
// member function picks the non-template overload when the declarator is
// not a template-id, a template-id or deduction picks the member template,
// and the explicit instantiation need not repeat the noexcept-specifier.
//@ expect: compile-ok
//@ strict-diags
template <class C> struct S {
  template <class T> constexpr int f(const T &) { return 1; }
  constexpr int f(const C *) { return 2; }
  constexpr int g() noexcept { return 3; }
};
extern template int S<char>::f(const char *);
template int S<char>::f(const char *);
template int S<char>::f<int>(const int &);
template int S<char>::f(const long &);
template int S<char>::g();
static_assert(S<char>{}.f("") == 2);
static_assert(S<char>{}.f(0) == 1);
