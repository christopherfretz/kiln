// [temp.deduct.call]/1 (CWG1391), [temp.deduct.general]/8: after deduction,
// an argument whose parameter type is non-dependent is checked for an
// implicit conversion before the deduced arguments are substituted into the
// function type; when it does not convert, deduction fails and the later
// parameters are not substituted. Here substituting `D` would instantiate a
// generic lambda's body with `std::nullptr_t` (a hard error). libc++
// `shared_ptr(nullptr_t, D, enable_if_t<__well_formed_deleter<D, nullptr_t>>)`
// for `shared_ptr<int> p(new int, deleter)` (LWG 4110 test). WP-296.
//@ expect: compile-ok
template <class D, class = decltype(D()(nullptr))> int t(int);
template <class D> char t(...);
template <class D> struct wf { static const bool value = sizeof(t<D>(0)) == sizeof(int); };
template <bool B, class T = void> struct en {};
template <class T> struct en<true, T> { using type = T; };
struct tag {};

struct S {
  int which;
  template <class D> S(int*, D) : which(1) {}
  template <class D> S(decltype(nullptr), D, typename en<wf<D>::value, tag>::type = tag()) : which(2) {}
};

template <class D> int g(long, D, typename en<wf<D>::value, tag>::type = tag()) { return 1; }
template <class D> int g(int*, D) { return 2; }

int main() {
  auto del = [](auto p) { delete p; };
  int i = 0;
  S s(&i, del);
  return s.which == 1 && g(&i, del) == 2 ? 0 : 1;
}
