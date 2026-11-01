// [class.friend]/2: friend classes, functions and templates may name private
// nested types; a friend of a derived class may not name a base's private ones.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:16 error "'Priv' is a private member of 'A'"
//@ diag: 10:10 note "implicitly declared private here"
//@ diag: 29:16 error "'Priv' is a private member of 'A'"
//@ diag: 10:10 note "implicitly declared private here"
class A {
  struct Priv { struct X { }; };
  typedef int I;
  friend struct F;
  friend void ff();
  template<class> friend struct FT;
  template<class T> friend void ft(T);
};
struct F {
  A::Priv p;
  A::I i;
  void m() { A::Priv::X x; (void)x; }
};
void ff() { A::Priv p; (void)p; }
template<class T> struct FT { typename A::Priv p; A::I i; };
FT<int> fti;
template<class T> void ft(T) { A::Priv p; (void)p; }
template void ft<int>(int);
class H : A { friend void hf(); };
void hf() { H::Priv p; (void)p; }
struct NF { A::Priv p; };
