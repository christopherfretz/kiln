// [temp.dep.type]/1, /6: `typename A::B<P>::C::m` and `typename X<P>::C::m`
// written before the declarator-id of an out-of-line member of nested class
// `C` of the current instantiation name the member `m` of `C`, so a static
// data member, a member function's return and parameter types match their
// in-class declarations; for a member class template of a non-template
// class, a class template and a partial specialization (WP-553).
//@ expect: compile-ok
//@ strict-diags

struct policy {
  template <class HP> struct apply {
    struct type {
      struct owner { int v; };
      static owner noone;
      static const owner* first();
      owner make(const owner&) const;
    };
  };
};
template <class HP> typename policy::apply<HP>::type::owner policy::apply<HP>::type::noone = {};
template <class HP> const typename policy::apply<HP>::type::owner* policy::apply<HP>::type::first() {
  return &noone;
}
template <class HP>
typename policy::apply<HP>::type::owner policy::apply<HP>::type::make(
    const typename policy::apply<HP>::type::owner& o) const {
  return o;
}

template <class T> struct outer {
  struct inner {
    struct leaf { int v; };
    static leaf j;
    leaf h(leaf) const;
  };
};
template <class T> typename outer<T>::inner::leaf outer<T>::inner::j = {3};
template <class T> typename outer<T>::inner::leaf outer<T>::inner::h(typename outer<T>::inner::leaf x) const {
  return x;
}

template <int N, class T> struct part;
template <class T> struct part<4, T> {
  struct inner {
    struct leaf { int v; };
    static leaf j;
    leaf h(leaf) const;
  };
};
template <class T> typename part<4, T>::inner::leaf part<4, T>::inner::j = {4};
template <class T> typename part<4, T>::inner::leaf part<4, T>::inner::h(typename part<4, T>::inner::leaf x) const {
  return x;
}

int use() {
  policy::apply<int>::type t;
  outer<int>::inner oi;
  part<4, int>::inner pi;
  return policy::apply<int>::type::noone.v + policy::apply<int>::type::first()->v + t.make({1}).v +
         outer<int>::inner::j.v + oi.h({1}).v + part<4, int>::inner::j.v + pi.h({2}).v;
}
