// [basic.stc.dynamic.deallocation]/5: within class `C` the first parameter
// of a destroying operator delete is `C *` (cv-unqualified pointee), and
// in a class template not a dependent type other than the
// injected-class-name's (clang 19 wording; see destroying-delete-first-param.cpp). WP-1040.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 25:8 error "first parameter of 'operator delete' must have type 'A *'"
//@ diag: 26:8 error "first parameter of 'operator delete' must have type 'A *'"
//@ diag: 30:8 error "'operator delete' cannot take a dependent type as first parameter; use 'D<T> *' instead"
//@ diag: 31:8 error "'operator delete' cannot take a dependent type as first parameter; use 'D<T> *' instead"
//@ diag: 33:6 error "first parameter of 'operator delete' must have type 'void *'"
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using std::size_t;
using std::align_val_t;
using dd_t = std::destroying_delete_t;
struct X;
struct A {
  void operator delete(X *, dd_t);
  void operator delete(const A *, dd_t, size_t);
};
template<class T> struct id { using type = T; };
template<class T> struct D {
  void operator delete(typename id<D>::type *, dd_t);
  void operator delete(T *, dd_t, size_t);
};
void operator delete(A *, dd_t);
