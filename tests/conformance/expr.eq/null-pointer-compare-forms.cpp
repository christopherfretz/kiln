// [conv.ptr]/1, [basic.compound]/3, [dcl.ref]/5: always-non-null comparisons
// (clang DiagnoseAlwaysNonNullPointer) in namespace-scope and constexpr
// initializers, default member initializers, unevaluated operands, lambdas,
// local scope and class template members: reference members and calls, default
// arguments and template-ids in the 'suffix with parentheses' note, macro
// arguments (warned) against macro bodies (not).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 65:33 warning "comparison of address of 'this->a' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 65:64 warning "comparison of address of 'this->a' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 66:12 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 67:13 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 68:14 warning "reference cannot be bound to dereferenced null pointer in well-defined C++ code; comparison may be assumed to always evaluate to false [-Wtautological-undefined-compare]"
//@ diag: 69:14 warning "comparison of address of 'm.v' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 70:12 warning "comparison of address of 'M::sv' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 71:13 warning "comparison of array 'm.fa' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 72:11 warning "comparison of function 'fd' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 72:11 note "prefix with the address-of operator to silence this warning"
//@ diag: 72:11 note "suffix with parentheses to turn this into a function call"
//@ diag: 73:11 warning "comparison of function 'fpd' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 73:11 note "prefix with the address-of operator to silence this warning"
//@ diag: 74:11 warning "comparison of function 'ft<int>' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 74:11 note "prefix with the address-of operator to silence this warning"
//@ diag: 74:11 note "suffix with parentheses to turn this into a function call"
//@ diag: 76:20 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 77:16 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 63:15 note "expanded from macro 'ID'"
//@ diag: 79:13 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 80:23 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 81:23 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 82:14 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 82:32 warning "comparison of array 'arr' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 83:89 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 84:38 warning "comparison of array 'arr' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 84:57 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 86:17 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 86:52 warning "comparison of array 'la' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 86:84 warning "comparison of address of 's' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 86:115 warning "comparison of address of 'lv' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 83:54 warning "comparison of address of 'this->t' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 85:19 note "in instantiation of member function 'C<int>::f' requested here"
//@ diag: 83:54 warning "comparison of address of 'this->t' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 85:36 note "in instantiation of member function 'C<long>::f' requested here"
//@ diag: 83:89 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 85:52 note "in instantiation of member function 'C<int>::g' requested here"
//@ diag: 83:89 warning "comparison of address of 'x' equal to a null pointer is always false [-Wtautological-pointer-compare]"
//@ diag: 85:69 note "in instantiation of member function 'C<long>::g' requested here"
//@ diag: 84:38 warning "comparison of array 'arr' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 85:76 note "in instantiation of function template specialization 'tt<int>' requested here"
//@ diag: 84:57 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 84:38 warning "comparison of array 'arr' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
//@ diag: 85:89 note "in instantiation of function template specialization 'tt<char>' requested here"
//@ diag: 84:57 warning "comparison of address of 'x' not equal to a null pointer is always true [-Wtautological-pointer-compare]"
int x, arr[3];
int &r = x;
struct M { int &rm; int v; static int sv; int fa[2]; };
M m{x, 1, {}};
int fd(int = 0);
int *fpd(int);
template<class T> T ft();
void ov(); void ov(int);
#define ID(e) e
#define CMP(a, b) ((a) == (b))
struct N { int a = 1; bool b = &a != nullptr; bool c = sizeof(&a == 0) > 0; };
bool e1 = &r == nullptr;
bool e2 = &(r) == nullptr;
bool e3 = &m.rm == 0;
bool e4 = &m.v != 0;
bool e5 = &M::sv == 0;
bool e6 = m.fa == 0;
bool e7 = fd == 0;
bool e8 = fpd == 0;
bool e9 = ft<int> == 0;
bool e10 = nullptr == 0;
bool e11 = sizeof(&x == 0) > 0;
bool e12 = ID(&x) == nullptr;
bool e13 = CMP(&x, nullptr);
bool e14 = &x == ID(nullptr);
constexpr bool e15 = &x != nullptr;
auto l = [] { return &x == nullptr; };
bool e16 = (&x == nullptr) == (arr == nullptr);
template<class T> struct C { T t; bool f() { return &t == nullptr; } bool g() { return &x == 0; } };
template<class T> bool tt() { return arr != nullptr && &x != 0; }
bool d = C<int>().f() && C<long>().f() && C<int>().g() && C<long>().g() && tt<int>() && tt<char>();
void h() { if (&x == nullptr) {} int la[2]; (void)(la == 0); static int s; (void)(&s != nullptr); int lv; (void)(&lv == nullptr); }
