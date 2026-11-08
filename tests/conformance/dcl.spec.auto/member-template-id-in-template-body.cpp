// [dcl.spec.auto], [over.over], [temp.res.general]: an `auto` variable
// initialized by a member template-id inside a template body (WP-833).
// clang resolves what it can at the definition: a non-static member
// template named through the current instantiation (`F::tm<long>`) is
// "must be called" there, with the template as a possible target and no
// hint to call it (the access is type-dependent, so clang's
// tryExprAsCall tries no call), and an instantiation does not report it
// again; named without an object from a static member function it is
// "call to non-static member function without an object argument" with
// no conversion diagnostic. An overload set of static member templates
// (`F::ov<long>`) is diagnosed only at instantiation, with the
// instantiation note; a template that is never instantiated is not
// diagnosed for it (the `N` shapes below).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 35:27 error "reference to non-static member function must be called"
//@ diag: 33:26 note "possible target for call"
//@ diag: 37:34 error "call to non-static member function without an object argument"
//@ diag: 40:47 error "call to non-static member function without an object argument"
//@ diag: 41:35 error "variable 'p' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 63:34 error "call to non-static member function without an object argument"
//@ diag: 64:39 error "call to non-static member function without an object argument"
//@ diag: 65:35 error "call to non-static member function without an object argument"
//@ diag: 66:27 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 62:26 note "possible target for call"
//@ diag: 66:20 error "variable has incomplete type 'void'"
//@ diag: 36:20 error "variable 'p' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 42:38 note "in instantiation of member function 'F<int>::m2' requested here"
template<class U> struct F {
  template<class T> static void ov(T);
  template<class T> static void ov(T, int);
  template<class T> void tm();
  template<class T> static void ts();
  void m1() { auto p = F::tm<long>; }
  void m2() { auto p = F::ov<long>; }
  static void m3() { auto p = F::tm<long>; }
  void m4() { auto p = F::ts<long>; }
};
template<class U> void g() { auto p = F<int>::tm<long>; }
template<class U> void h() { auto p = F<int>::ov<long>; }
void use() { F<int>().m1(); F<int>().m2(); F<int>::m3(); }

template<class U> struct N {
  template<class T> static void ov(T);
  template<class T> static void ov(T, int);
  static void s();
  static void s(int);
  template<class T> void mx(T);
  template<class T> static void mx(T, int);
  void a1() { auto p = ov<long>; }
  void a2() { auto p = this->ov<long>; }
  void a3() { auto p = N<U>::ov<long>; }
  void a4() { auto p = s; }
  void a5() { auto p = N::s; }
  void a6() { auto p = mx<long>; }
  static void a7() { auto p = ov<long>; }
  void a8() { auto p{N::ov<long>}; }
};

struct G {
  template<class T> void tm();
  static void s1() { auto p = G::tm<long>; }
  static void s2() { void (*p)() = G::tm<long>; }
  static void s3() { auto &p = G::tm<long>; }
  void s4() { auto p = G::tm<long>; }
};
