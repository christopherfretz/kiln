// [expr.delete]/10, [dcl.fct.def.delete]/2: the deallocation function a
// delete-expression selects is used, so a deleted one is an error; a
// destroying operator delete, when the class has one, is the only
// candidate, chosen by alignment then size like the others. A destroying
// delete still needs an accessible virtual destructor ([expr.delete]/12).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:17 error "attempt to use a deleted function"
//@ diag: 28:17 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 34:17 error "attempt to use a deleted function"
//@ diag: 32:8 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 39:17 error "attempt to use a deleted function"
//@ diag: 36:8 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 44:17 error "attempt to use a deleted function"
//@ diag: 42:8 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 55:24 error "calling a protected destructor of class 'T'"
//@ diag: 53:11 note "declared protected here"
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using std::size_t;
using std::align_val_t;
using dd_t = std::destroying_delete_t;
struct X { void operator delete(void *) = delete; };
void f1(X *x) { delete x; }
struct B {
  void operator delete(void *);
  void operator delete(B *, dd_t) = delete;
};
void f2(B *b) { delete b; }
struct D {
  void operator delete(D *, dd_t) = delete;
  void operator delete(D *, dd_t, align_val_t);
};
void f3(D *d) { delete d; }
struct alignas(__STDCPP_DEFAULT_NEW_ALIGNMENT__ * 2) E {
  void operator delete(E *, dd_t);
  void operator delete(E *, dd_t, align_val_t) = delete;
};
void f4(E *e) { delete e; }
struct F {
  void operator delete(F *, dd_t, size_t) = delete;
  void operator delete(F *, dd_t);
};
void f5(F *f) { delete f; }
struct T {
  void operator delete(T *, dd_t);
protected:
  virtual ~T();
};
void f6(T *t) { delete t; }
