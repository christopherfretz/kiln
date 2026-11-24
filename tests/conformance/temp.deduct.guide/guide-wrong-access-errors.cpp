// [temp.deduct.guide]/3: a deduction guide for a member class template
// has the same access as the template. A guide declared with another
// access is rejected at its first token (`explicit` when written), noting
// the template's `template` keyword and the access-specifier that gave
// the guide its access; also in a class template's definition. A guide
// with the template's access is valid.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 22:75 error "deduction guide has different access from the corresponding member template"
//@ diag: 22:11 note "member template declared private here"
//@ diag: 22:49 note "deduction guide declared public by intervening access specifier"
//@ diag: 27:21 error "deduction guide has different access from the corresponding member template"
//@ diag: 25:3 note "member template declared public here"
//@ diag: 26:1 note "deduction guide declared private by intervening access specifier"
//@ diag: 35:3 error "deduction guide has different access from the corresponding member template"
//@ diag: 31:3 note "member template declared private here"
//@ diag: 34:1 note "deduction guide declared protected by intervening access specifier"
//@ diag: 40:21 error "deduction guide has different access from the corresponding member template"
//@ diag: 38:3 note "member template declared public here"
//@ diag: 39:1 note "deduction guide declared private by intervening access specifier"
class P { template<class T> struct M { M(T); }; public: template<class T> M(T) -> M<T>; };
class Q {
public:
  template<class T> struct M { M(T); };
private:
  template<class T> explicit M(T) -> M<T>;
};
struct R {
private:
  template<class T> struct M { M(T); };
public:
  int x;
protected:
  M(int) -> M<int>;
};
template<class T> struct O {
  template<class U> struct M { M(U); };
private:
  template<class U> M(U) -> M<U>;
};
// Valid: the same access, also after an access-specifier that repeats it.
class V {
  template<class T> struct M { M(T); };
  template<class T> M(T) -> M<T>;
public:
  int y;
private:
  M(int) -> M<int>;
};
// The class template's guide is diagnosed once, not again when
// instantiated.
O<int> oi;
