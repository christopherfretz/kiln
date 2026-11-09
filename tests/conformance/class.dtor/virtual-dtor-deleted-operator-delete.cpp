// [class.dtor]/16: the deallocation function a virtual destructor looks up
// is used by its definition, so a deleted one makes the definition
// ill-formed; [class.dtor]/7.3: an implicit or defaulted virtual destructor
// whose lookup finds a deleted one is itself deleted. A non-virtual
// destructor, a deleted virtual destructor and an uninstantiated template
// are fine. Found by the clang negative-test run (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:11 error "attempt to use a deleted function"
//@ diag: 24:8 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 30:5 error "attempt to use a deleted function"
//@ diag: 28:8 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 32:11 warning "explicitly defaulted destructor is implicitly deleted"
//@ diag: 31:8 note "virtual destructor requires an unambiguous, accessible 'operator delete'"
//@ diag: 32:19 note "replace 'default' with 'delete'"
//@ diag: 38:8 error "deleted function '~D5' cannot override a non-deleted function"
//@ diag: 36:11 note "overridden virtual function is here"
//@ diag: 38:8 note "virtual destructor requires an unambiguous, accessible 'operator delete'"
//@ diag: 44:12 error "attempt to use a deleted function"
//@ diag: 42:8 note "'operator delete' has been explicitly marked deleted here"
struct D {
  virtual ~D() {}
  void operator delete(void *) = delete;
};
struct D2 {
  virtual ~D2();
  void operator delete(void *) = delete;
};
D2::~D2() {}
struct D3 {
  virtual ~D3() = default;
  void operator delete(void *) = delete;
};
struct B4 {
  virtual ~B4();
};
struct D5 : B4 {
  void operator delete(void *) = delete;
};
struct N {
  void operator delete(void *) = delete;
};
void g() { new N; }

struct OK1 {
  virtual ~OK1() {}
  void operator delete(void *);
};
struct OK2 {
  ~OK2() {}
  void operator delete(void *) = delete;
};
struct OK3 {
  virtual ~OK3() = delete;
  void operator delete(void *) = delete;
};
template <class T> struct OK4 {
  virtual ~OK4() {}
  void operator delete(void *) = delete;
};
