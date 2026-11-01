// [basic.start.dynamic]/2, [stmt.dcl]/3: libc++ support/count_new.h:377.
// A namespace-scope reference initialized from a call is dynamically
// initialized; the inline function's static local (direct-initialized
// with a parenthesized temporary, the WP-175 parse) is guarded in the
// function's comdat.
struct MemCounter {
  struct MemCounterCtorArg_ {};
  explicit MemCounter(MemCounterCtorArg_) {}
  int outstanding = 0;
  int new_called = 0;
};
inline MemCounter* getGlobalMemCounter() {
  static MemCounter counter((MemCounter::MemCounterCtorArg_()));
  return &counter;
}
MemCounter &globalMemCounter = *getGlobalMemCounter();
int use() { return globalMemCounter.new_called; }
