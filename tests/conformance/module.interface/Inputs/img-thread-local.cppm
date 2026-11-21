// The module of image-thread-local-run.cpp: every form of thread
// storage duration an interface can export, with constant and dynamic
// initializers, defined here once.
export module imgtl;
extern "C" int printf(const char *, ...);
int trace(int v) { return v; }
export thread_local int k = 3;
export thread_local int d = trace(4);
export extern thread_local int ext;
thread_local int ext = trace(5);
export __thread int st = 6;
export _Thread_local int c11 = 7;
export [[gnu::tls_model("initial-exec")]] thread_local int ie = trace(8);
export constinit thread_local int ci = 9;
export constexpr thread_local int cx = 10;
export thread_local const int cc = trace(11);
export thread_local int arr[3] = {1, trace(12), 3};
export thread_local int &ref = arr[1];
thread_local int hidden = trace(13);
export inline thread_local int inl = trace(14);
export struct S {
  static thread_local int sm;
  static inline thread_local int si = trace(15);
};
thread_local int S::sm = trace(16);
export struct T {
  int v;
  T() : v(trace(17)) {}
  ~T() { printf("~T %d\n", v); }
};
export thread_local T t;
export namespace ns {
thread_local int nested = trace(18);
}
export inline int counter() {
  static thread_local int n = 0;
  return ++n;
}
export inline int get_hidden() { return hidden; }
export int *module_d() { return &d; }
