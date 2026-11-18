// [temp.expl.spec]/15 with [temp.friend]/1: the template-heads of a
// declaration whose declarator-id is qualified by a class template
// specialization (`A<int>::`) are matched to the qualifier's classes
// first, and an implicitly instantiated specialization matches only
// `template<>`, which a friend declaration cannot have: a friend
// naming a member of `A<int>`, or of a class nested in it, takes no
// head for `A<int>`, so `template<class U>` before it is clang 22's
// error (and declares nothing), also for a constructor and a member
// template, whose own head would come after a `template<>`. GCC 16.1
// rejects the constructor and the member templates ("no declaration
// matches") and accepts the non-template members, which the text does
// not allow. The accepted forms are
// temp.friend/member-of-specialization-friend-run.cpp.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 38:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 39:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 40:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 41:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 42:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"
//@ diag: 46:3 error "template parameter list matching the non-templated nested type 'A<int>' should be empty ('template<>')"

template <class T> struct A {
  A(int *);
  template <class U> A(U *);
  void f();
  template <class U> void g(U);
  struct N {
    void h();
    template <class U> void k(U);
  };
};

class B {
  template <class U> friend A<int>::A(U *);
  template <class U> friend void A<int>::f();
  template <class U> friend void A<int>::g(U);
  template <class U> friend void A<int>::N::h();
  template <class U> friend void A<int>::N::k(U);
  template <class U> template <class W> friend void A<int>::g(U);
};

template <class V> class C {
  template <class U> friend void A<int>::g(U);
};
C<int> c;
