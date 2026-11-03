// [temp.constr.atomic]/3, [temp.dep.type]/9, [temp.arg.general]/9: a
// concept-id whose argument names a block-scope constant of a function
// template (`TC<N>`, `N` a `constexpr` local) is instantiation-dependent but
// not dependent, so it is checked at the template's definition against the
// argument's canonical form. Its atoms must see the concrete argument: a
// nested concept-id over a class template member (`IS<T, U>::value`) is
// neither assumed satisfied nor cached so, and a non-type template
// parameter's default argument that is such a concept-id is a value
// (libc++ subrange's `_Kind`, list.cons/from_range.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
template <int N> struct TC { int x; };
template <class T, class U> struct IS { static constexpr bool value = __is_same(T, U); };
template <class T, class U> concept SI = IS<T, U>::value;
template <class T> concept NotBool = !SI<T, bool>;
template <class T> concept Ptr8 = sizeof(T) == sizeof(void*);

template <class D> struct Base {};
template <class I, bool K = Ptr8<I>>
struct Range : Base<Range<I, K>> { static constexpr bool sized = K; };

template <class U> void f() {
  constexpr int N = 3;
  using T = TC<N>;
  static_assert(NotBool<T*>);
  static_assert(!SI<TC<N>*, bool>);
  static_assert(SI<T*, TC<3>*>);
  Range<T*> r;
  static_assert(decltype(r)::sized);
  [](T* p) { Range<T*> q; (void)p; (void)q; }(nullptr);
}
template void f<int>();

// The definition-time check must not leave a wrong answer behind for the
// same canonical arguments.
static_assert(NotBool<TC<3>*>);
static_assert(Range<TC<3>*>::sized);
int main() {}
