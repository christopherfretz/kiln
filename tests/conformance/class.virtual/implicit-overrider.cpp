// [class.virtual]/4, /8, [class.copy.assign]/6, [class.dtor]/4: an
// implicitly declared copy or move assignment operator or destructor that
// overrides a base virtual function is an overrider like any other. It may
// not override a 'final' function, and its return type (D& for the
// assignment operators) must match or be covariant with the overridden
// one. Clang reports both at the class name, with the note "while declaring
// the implicit <member> for 'D'" on the first error of each declaration.
// Found by the clang negative-test run (WP-957), fixed by WP-1078. Valid
// twin: implicit-overrider-ok.cpp.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-final-dtor-non-final-class
//@ diag: 48:8 error "declaration of '~DA' overrides a 'final' function"
//@ diag: 48:8 note "while declaring the implicit destructor for 'DA'"
//@ diag: 47:20 note "overridden virtual function is here"
//@ diag: 56:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 56:8 note "while declaring the implicit copy assignment operator for 'D1'"
//@ diag: 53:15 note "overridden virtual function is here"
//@ diag: 57:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 57:8 note "while declaring the implicit move assignment operator for 'D2'"
//@ diag: 54:15 note "overridden virtual function is here"
//@ diag: 62:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 62:8 note "while declaring the implicit copy assignment operator for 'D3'"
//@ diag: 60:24 note "overridden virtual function is here"
//@ diag: 62:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 61:24 note "overridden virtual function is here"
//@ diag: 65:27 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 65:27 note "while declaring the implicit copy assignment operator for 'D4<int>'"
//@ diag: 66:9 note "in instantiation of template class 'D4<int>' requested here"
//@ diag: 64:43 note "overridden virtual function is here"
//@ diag: 70:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 70:8 note "while declaring the implicit copy assignment operator for 'D5'"
//@ diag: 69:24 note "overridden virtual function is here"
//@ diag: 70:8 error "deleted function 'operator=' cannot override a non-deleted function"
//@ diag: 69:24 note "overridden virtual function is here"
//@ diag: 70:27 note "copy assignment operator of 'D5' is implicitly deleted because field 'c' is of const-qualified type 'const int'"
//@ diag: 74:8 error "virtual function 'operator=' has a different return type ('D6 &') than the function it overrides (which has return type 'int')"
//@ diag: 74:8 note "while declaring the implicit copy assignment operator for 'D6'"
//@ diag: 73:24 note "overridden virtual function is here"
//@ diag: 78:8 error "virtual function 'operator=' has a different return type ('D7 &') than the function it overrides (which has return type 'int')"
//@ diag: 78:8 note "while declaring the implicit copy assignment operator for 'D7'"
//@ diag: 77:24 note "overridden virtual function is here"
//@ diag: 78:8 error "declaration of 'operator=' overrides a 'final' function"
//@ diag: 77:24 note "overridden virtual function is here"

struct A { virtual ~A() final; };
struct DA : A {};

struct D1;
struct D2;
struct B {
  virtual D1 &operator=(const D1 &) final;
  virtual D2 &operator=(D2 &&) final;
};
struct D1 : B {};
struct D2 : B {};

struct D3;
struct X { virtual D3 &operator=(const D3 &) final; };
struct Y { virtual D3 &operator=(const D3 &) final; };
struct D3 : X, Y {};

template <class T> struct TB { virtual T &operator=(const T &) final; };
template <class T> struct D4 : TB<D4<T>> {};
D4<int> d4;

struct D5;
struct W { virtual D5 &operator=(const D5 &) final; };
struct D5 : W { const int c = 0; };

struct D6;
struct R { virtual int operator=(const D6 &); };
struct D6 : R {};

struct D7;
struct Q { virtual int operator=(const D7 &) final; };
struct D7 : Q {};
