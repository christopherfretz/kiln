// [temp.inject]/1, [temp.friend]/1, [temp.inst]/4: a friend function
// template declared without a body in one class template specialization
// and defined in a specialization of another class template is one
// template; once the defining specialization is instantiated, its
// specializations, also one named before the definition existed, are
// instantiated from that definition with the defining class's template
// arguments. Before it is instantiated, the template has no definition: a
// requires-expression naming a specialization with a deduced return type
// is not satisfied. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
template<class T> struct slot {
  template<class U> friend constexpr int alloc(slot<T>, U);
};
template<class T> struct setter {
  template<class U> friend constexpr int alloc(slot<T *>, U) {
    return int(sizeof(T)) * 10 + int(sizeof(U));
  }
};
template struct slot<long *>;
template struct setter<long>;
static_assert(alloc(slot<long *>{}, 'c') == 81, "");
template<int N> struct R {
  template<class> friend constexpr auto flag(R<N>);
};
template<int N> struct W {
  template<class> friend constexpr auto flag(R<N>) { return N; }
};
template<int N, int Unique> constexpr bool has() {
  return requires { flag<void>(R<N>{}); };
}
static_assert(!has<1, 0>(), "");
template struct W<1>;
static_assert(has<1, 1>(), "");
static_assert(flag<void>(R<1>{}) == 1, "");
