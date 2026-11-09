// [expr.delete]/6, CWG2889: a delete-expression that selects a destroying
// operator delete does not invoke a non-virtual destructor, so that
// destructor need not be accessible (clang 20+; the -19 sibling pins clang
// 19's error). WP-1040.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
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
