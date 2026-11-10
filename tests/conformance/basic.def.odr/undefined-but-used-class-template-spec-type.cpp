// [basic.link]/15, [basic.link]/8, [basic.def.odr]/11: a type has
// linkage if it is a class template specialization (whatever its template
// arguments) or a member of one, so an undefined odr-used specialization
// whose type is `W<L>` or `W<L>::N` for a local class `L` is not the
// no-linkage-type error but `-Wundefined-internal` (it takes no linkage
// from its template argument); one whose type is `L` itself, a pointer to
// it or a function of it is the error. Matches clang 19 and 22 (WP-1224).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-undefined-var-template
//@ diag: 24:29 error "variable 'vT<L>' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 30:40 note "used here"
//@ diag: 25:32 warning "variable 'vW<L>' has internal linkage but is not defined"
//@ diag: 30:54 note "used here"
//@ diag: 26:44 warning "variable 'vN<L>' has internal linkage but is not defined"
//@ diag: 30:68 note "used here"
//@ diag: 27:25 error "function 'fT<L>' is used but not defined in this translation unit, and cannot be defined in any other translation unit because its type does not have linkage"
//@ diag: 31:3 note "used here"
//@ diag: 28:25 warning "function 'fW<L>' has internal linkage but is not defined"
//@ diag: 31:12 note "used here"
//@ diag: 29:25 warning "function 'fN<L>' has internal linkage but is not defined"
//@ diag: 31:24 note "used here"
template <class T> struct W { T v; struct N {}; };
template <class T> extern T vT;
template <class T> extern W<T> vW;
template <class T> extern typename W<T>::N vN;
template <class T> void fT(T);
template <class T> void fW(W<T>);
template <class T> void fN(typename W<T>::N);
template <class T> void use() { (void)&vT<T>; (void)&vW<T>; (void)&vN<T>;
  fT(T{}); fW(W<T>{}); fN<T>({}); }
int main() {
  struct L {};
  use<L>();
}
