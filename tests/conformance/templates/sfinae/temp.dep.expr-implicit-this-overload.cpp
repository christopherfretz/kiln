//@ expect: run
// [temp.dep.expr]/3: `this` is type-dependent in a member of a class
// template, so an implicit member call `f()` of an overloaded member is
// resolved at instantiation, like `this->f()`. Before then the overloads
// that differ only in a dependent `enable_if` cannot be told apart
// (libc++ `__shared_ptr_emplace::__on_zero_shared_impl`).
template <bool B, class T = void> struct EI {};
template <class T> struct EI<true, T> { using type = T; };
template <bool B, class T = void> using EI_t = typename EI<B, T>::type;
struct Base { virtual int z() = 0; };
template <class A> struct S : Base {
  template <class X = A, EI_t<sizeof(X) == 4, int> = 0> int impl() { return 1; }
  template <class X = A, EI_t<sizeof(X) != 4, int> = 0> int impl() { return 2; }
  int z() override { return impl(); }
  int w() { return impl() + this->impl() + S::impl(); }
};
int main() {
  S<int> a;
  S<double> b;
  return a.z() == 1 && b.z() == 2 && a.w() == 3 && b.w() == 6 ? 0 : 1;
}
