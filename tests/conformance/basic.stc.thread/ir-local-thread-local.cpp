// [basic.stc.thread]/1, [stmt.dcl]/3: a block-scope `thread_local`
// variable (with or without `static`) is an `internal thread_local`
// global named as a function-local static (`@_ZZ1gvE1i`), reached
// through `llvm.threadlocal.address`; one with dynamic initialization has
// a thread-local guard and registers its destructor with
// `__cxa_thread_atexit`. The IR equals clang's.
//@ expect: ir
//@ requires: clang>=22
struct S {
  S();
  ~S();
  int v;
};
int& g() { thread_local int i = 0; return i; }
int& h() { static thread_local int j = 5; return j; }
int* p() { thread_local int k = 1; return &k; }
int k() { thread_local S s; return s.v; }
