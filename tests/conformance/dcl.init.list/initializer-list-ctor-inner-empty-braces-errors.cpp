// [dcl.init.list]/3.7, [dcl.init.list]/5: in `{{}}` initializing an
// initializer list, the inner `{}` copy-list-initializes its one element,
// so an element type whose default constructor is deleted makes the
// initialization ill-formed; the list is not aggregate-initialized from
// the inner braces, even when std::initializer_list is an aggregate.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:31 error "call to deleted constructor of 'const D'"
//@ diag: 18:12 note "'D' has been explicitly marked deleted here"
typedef decltype(sizeof(char)) size_t;
namespace std {
template<class T> struct initializer_list {
  const T *b_;
  size_t n_;
};
}
struct D { D() = delete; };
std::initializer_list<D> l = {{}};
