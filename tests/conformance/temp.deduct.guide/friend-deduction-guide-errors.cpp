// [temp.deduct.guide]/3: a deduction guide inhabits the scope of its
// class template; a friend declaration cannot declare one. clang's
// "cannot declare a deduction guide as a friend" at the template name,
// in a class and in a class template (nothing is declared) (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:37 error "cannot declare a deduction guide as a friend"
//@ diag: 13:55 error "cannot declare a deduction guide as a friend"

template<class T> struct D { D(T); };
D(int) -> D<int>;
struct G { template<class T> friend D(T) -> D<T>; };
template<class T> struct H { template<class U> friend D(U) -> D<U>; };
H<int> h;
D d(1);
