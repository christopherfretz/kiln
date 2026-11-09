// Interface unit of module `ts` for the module.interface two-step tests:
// a non-inline exported function and variable defined only here, an
// exported inline function, inline variable and template, and a class
// with an out-of-line member.
export module ts;
export int ts_calls = 0;
export int ts_answer() { return ++ts_calls + 41; }
export inline int ts_twice(int x) { return 2 * x; }
export inline int ts_shared = 10;
export template <class T> T ts_sum(T a, T b) { return a + b; }
export template <class T> struct Box {
  T v;
  T get() const { return v; }
};
export struct Counter {
  int n = 0;
  int bump();
};
int Counter::bump() { return ++n; }
