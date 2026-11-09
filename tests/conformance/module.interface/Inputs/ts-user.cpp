// The second importer of ts (two-step-*-run.cpp): it uses the same
// entities as the test file, so any definition both emitted (or the
// interface and an importer both emitted) would collide at link time.
import ts;
int ts_user() {
  Counter c;
  Box<int> b{3};
  ts_shared += 1;
  return ts_answer() + ts_twice(2) + ts_sum(1, 2) + b.get() + c.bump();
}
int *ts_shared_addr() { return &ts_shared; }
