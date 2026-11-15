// [over.over]/5 (CWG2918): nothing satisfied, or several candidates of
// which none is more partial-ordering-constrained, is ill-formed; members
// of different types leave the placeholder undeduced
// ([temp.deduct.call]/6).
//@ expect: compile-fail
//@ requires: clang>=22
//@ diag: 16:16 error "address of overloaded function 'g' does not match required type 'void ()'"
//@ diag: 17:6 error "variable 'c' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
template <bool B> struct X {
  static void f(short) requires B;
  static void f(long);
  static void g() requires (!B);
  static void g() requires (!B && true);
};
void (*a)(short) = &X<true>::f;
void (*b)() = &X<true>::g;
auto c = &X<true>::f;
