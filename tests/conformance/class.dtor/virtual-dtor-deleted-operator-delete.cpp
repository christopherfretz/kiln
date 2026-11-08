// [class.dtor]/16: a virtual destructor's non-array deallocation function
// lookup that finds a deleted function makes the program ill-formed. Found
// by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, deleted operator delete for virtual destructors is not checked
//@ expect: compile-fail
//@ diag: 9:11 error "attempt to use a deleted function"
struct D {
  virtual ~D() {}
  void operator delete(void *) = delete;
};
