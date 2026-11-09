// [basic.stc.dynamic.deallocation]/5: a destroying operator delete is a
// usual deallocation function: after `C *` and `std::destroying_delete_t`
// only an optional `std::size_t` then an optional `std::align_val_t`, and
// not a template. Checked when the class is not dependent, so a class
// template's member is checked at instantiation (WP-1040).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:8 error "destroying operator delete can have only an optional size and optional alignment parameter"
//@ diag: 26:8 error "destroying operator delete can have only an optional size and optional alignment parameter"
//@ diag: 27:15 error "destroying operator delete can have only an optional size and optional alignment parameter"
//@ diag: 29:35 error "destroying operator delete can have only an optional size and optional alignment parameter"
//@ diag: 30:35 error "destroying operator delete can have only an optional size and optional alignment parameter"
//@ diag: 31:8 note "in instantiation of template class 'C<int>' requested here"
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using std::size_t;
using std::align_val_t;
using dd_t = std::destroying_delete_t;
struct A {
  void operator delete(A *, dd_t, int);
  void operator delete(A *, dd_t, align_val_t, size_t);
  static void operator delete(A *, dd_t, size_t, align_val_t, int);
};
struct B { template<class T> void operator delete(B *, dd_t, T); };
template<class T> struct C { void operator delete(C *, dd_t, T); };
C<int> c;
