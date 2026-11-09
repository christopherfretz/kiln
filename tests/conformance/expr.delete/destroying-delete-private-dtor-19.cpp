// [expr.delete]/6, CWG2889: a delete-expression that selects a destroying
// operator delete does not invoke a non-virtual destructor, so that
// destructor need not be accessible (clang 20+; this pins clang 19, which
// still checks it; see destroying-delete-private-dtor.cpp). WP-1040.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 23:23 error "calling a private destructor of class 'S'"
//@ diag: 21:3 note "declared private here"
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using dd_t = std::destroying_delete_t;
struct S {
  void operator delete(S *p, dd_t);
private:
  ~S();
};
void f(S *s) { delete s; }
