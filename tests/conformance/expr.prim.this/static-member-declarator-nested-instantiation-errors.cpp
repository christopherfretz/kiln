// [expr.prim.this]/3: rejected side of
// static-member-declarator-nested-instantiation.cpp. A static member
// function's own trailing return type still may not name a non-static
// member without an object after it has instantiated a class whose
// members use `this`, at the definition and in each instantiation of a
// class template's member.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:83 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 27:74 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 31:78 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 31:78 error "'this' cannot be implicitly used in a static member function declaration"
//@ diag: 33:13 note "in instantiation of template class 'holder<int>' requested here"
template <class T> T&& declval();
template <class T> struct exact { T payload; };

template <class C> struct stream {
  C buffer[4];
  template <class T> bool put(exact<T>, C*);
  template <class T> auto stream_in(exact<T> x) -> decltype(put(x, buffer));
};

struct plain {
  int m;
  static auto get() -> decltype(declval<stream<char>&>().stream_in(exact<int>()), m);
  template <class T> static auto t(T) -> decltype(declval<stream<T>&>(), m);
};
template <class U> struct holder {
  int m;
  static auto get() -> decltype(declval<stream<U>&>().stream_in(exact<U>()), m);
};
holder<int> h;
