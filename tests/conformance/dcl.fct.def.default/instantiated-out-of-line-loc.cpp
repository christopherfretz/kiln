// [dcl.fct.def.default]/5, [temp.inst]/11: a member of a class template
// defaulted out of line is implicitly defined when it is instantiated, at
// its out-of-line `= default` definition (clang
// `InstantiateFunctionDefinition` locates the instantiation at the pattern
// definition). A subobject that cannot be initialized or destroyed is
// diagnosed there, and the "in defaulted ... first required here" note names
// the definition too. A defaulted assignment operator reports at the end of
// its declarator (clang `DefineImplicitCopyAssignment` uses `getEndLoc()`);
// an out-of-line definition skips the deletion checks, so a deleted member
// `operator=` is "attempt to use a deleted function" ([dcl.fct.def.delete]/2).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:7 error "call to deleted constructor of 'NC'"
//@ diag: 44:7 note "in defaulted copy constructor for 'C<int>' first required here"
//@ diag: 45:28 note "in instantiation of member function 'C<int>::C' requested here"
//@ diag: 37:19 note "'NC' has been explicitly marked deleted here"
//@ diag: 48:25 error "constructor for 'E<int>' must explicitly initialize the reference member 'r'"
//@ diag: 48:25 note "in defaulted default constructor for 'E<int>' first required here"
//@ diag: 49:8 note "in instantiation of member function 'E<int>::E' requested here"
//@ diag: 47:38 note "declared here"
//@ diag: 53:25 error "field of type 'P' has private default constructor"
//@ diag: 53:25 note "in defaulted default constructor for 'F<int>' first required here"
//@ diag: 54:8 note "in instantiation of member function 'F<int>::F' requested here"
//@ diag: 51:21 note "declared private here"
//@ diag: 61:31 error "attempt to use a deleted function"
//@ diag: 61:13 note "in defaulted copy assignment operator for 'A<int>' first required here"
//@ diag: 62:34 note "in instantiation of member function 'A<int>::operator=' requested here"
//@ diag: 55:17 note "'operator=' has been explicitly marked deleted here"
//@ diag: 65:39 error "field of type 'ND' has private destructor"
//@ diag: 66:25 note "in defaulted destructor for 'D<int>' first required here"
//@ diag: 67:21 note "in instantiation of member function 'D<int>::~D' requested here"
//@ diag: 64:22 note "declared private here"
//@ diag: 71:25 error "field of type 'MV' has private move constructor"
//@ diag: 71:25 note "in defaulted move constructor for 'G<int>' first required here"
//@ diag: 72:28 note "in instantiation of member function 'G<int>::G' requested here"
//@ diag: 69:28 note "declared private here"
struct NC { NC(); NC(const NC&) = delete; };
template<class T> struct C {
  C(const C&);
  T t;
  NC n;
};
template<class T>
C<T>::C(const C&) = default;
void f(C<int>& a) { C<int> b(a); }

template<class T> struct E { E(); T& r; };
template<class T> E<T>::E() = default;
E<int> e;

struct P { private: P(); };
template<class T> struct F { F(); P p; };
template<class T> F<T>::F() = default;
F<int> fi;
struct NA { NA& operator=(const NA&) = delete; };
template<class T> struct A {
  A& operator=(const A&);
  NA n;
};
template<class T>
A<T>& A<T>::operator=(const A&) = default;
void g(A<int>& a, A<int>& b) { a = b; }

struct ND { private: ~ND(); };
template<class T> struct D { ~D(); ND n; };
template<class T> D<T>::~D() = default;
void h(D<int>* p) { delete p; }

struct MV { MV(); private: MV(MV&&); };
template<class T> struct G { G(G&&); MV m; };
template<class T> G<T>::G(G&&) = default;
void k(G<int>& x) { G<int> y(static_cast<G<int>&&>(x)); }
