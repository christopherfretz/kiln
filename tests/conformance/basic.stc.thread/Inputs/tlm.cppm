// Interface unit for module-tls-run.cpp.
module;
extern "C" int printf(const char *, ...);
export module tlm;
int counter = 0;
int next() { return ++counter; }
export struct T {
  int v;
  T() : v(next() * 10) {}
  ~T() { printf("~T %d\n", v); }
};
export thread_local int x = next();
export thread_local int k = 3;
export thread_local T t;
thread_local int y = 100 + next();
export int gety() { return y; }
