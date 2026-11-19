// [expr.prim.this]/2-3: the rule against `this` in a static member
// function's declaration judges that declaration only. A class template
// instantiated from a static member function's trailing return type, a
// non-static member template's signature deduced there, and a member
// function body instantiated there for its deduced return type each use
// `this` of their own, as the non-static members they are. Reduced from
// Boost.LexicalCast (a static member template's trailing return type
// calling a non-static `stream_in` whose trailing return type names a data
// member), which kiln 1.2.1 accepted.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
template <class T> T&& declval();
template <class T> struct exact { T payload; };

template <class C> struct stream {
  C buffer[4];
  template <class T> bool put(exact<T>, C*);
  template <class T> auto stream_in(exact<T> x) -> decltype(put(x, buffer));
  auto size() -> decltype(put(exact<int>(), buffer));
  auto data() { return buffer; }
};

// The class template is instantiated from a static member template's
// trailing return type, while the declarations of a class template's
// members are instantiated (Boost's `lexical_converter_impl`).
template <class T> struct converter {
  template <class U>
  static auto detect(int)
      -> decltype(declval<stream<char>&>().stream_in(declval<exact<U>>()), 0);
  template <class U> static long detect(...);
  using type = decltype(detect<T>(1));
};
static_assert(__is_same(converter<double>::type, int));

// The same from a non-template class's static member function.
struct plain {
  static auto size() -> decltype(declval<stream<short>&>().size());
};
static_assert(__is_same(decltype(plain::size()), bool));

// Deduction of a non-static member template of a complete class template
// specialization.
template struct stream<long>;
struct deduce {
  static auto get() -> decltype(declval<stream<long>&>().stream_in(exact<int>()));
};
static_assert(__is_same(decltype(deduce::get()), bool));

// A member function body instantiated for its deduced return type.
struct body {
  static auto get() -> decltype(declval<stream<unsigned>&>().data());
};
static_assert(__is_same(decltype(body::get()), unsigned*));
