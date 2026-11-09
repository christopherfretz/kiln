// C++03 [class.union]/1: a union member (or an anonymous struct's
// member) may not have a class type, or array thereof, with a
// non-trivial default constructor, copy constructor, copy assignment
// operator or destructor; the first of those is named, copy
// constructor first (clang CheckNontrivialField; WP-1042). clang's
// "because ..." notes are not produced (no strict-diags).
//@ expect: compile-fail
//@ std: c++98
//@ diag: 32:11 error "union member 'v' has a non-trivial copy constructor"
//@ diag: 33:9 error "union member 'vb' has a non-trivial copy constructor"
//@ diag: 34:8 error "union member 'c' has a non-trivial default constructor"
//@ diag: 35:11 error "union member 'oi' has a non-trivial default constructor"
//@ diag: 36:8 error "union member 'cp' has a non-trivial copy constructor"
//@ diag: 37:10 error "union member 'as' has a non-trivial copy assignment operator"
//@ diag: 38:8 error "union member 'd' has a non-trivial destructor"
//@ diag: 39:8 error "union member 'arr' has a non-trivial default constructor"
//@ diag: 40:10 error "union member 'h' has a non-trivial default constructor"
//@ diag: 44:17 error "anonymous struct member 'd' has a non-trivial destructor"
//@ diag: 45:16 error "union member 'cp' has a non-trivial copy constructor"
//@ diag: 47:39 error "union member 'b' has a non-trivial destructor"
//@ diag: 49:9 note "in instantiation of template class 'T<Dtor>' requested here"
struct Virtual { virtual void f(); };
struct VBase : virtual Virtual {};
struct Ctor { Ctor(); };
struct OnlyInt { OnlyInt(int); };
struct Copy { Copy(Copy &); };
struct Assign { Assign &operator=(Assign &); };
struct Dtor { ~Dtor(); };
struct Holder { Ctor c; };
struct Okay { int i; void f(); };
union U1 {
  Virtual v;
  VBase vb;
  Ctor c;
  OnlyInt oi;
  Copy cp;
  Assign as;
  Dtor d;
  Ctor arr[2][3];
  Holder h;
  Okay ok;
};
struct S {
  struct { Dtor d; int i; };
  union { Copy cp; int j; };
};
template <class B> union T { int a; B b; };
T<int> ti;
T<Dtor> td;
