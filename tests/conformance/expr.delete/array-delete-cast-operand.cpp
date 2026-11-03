// [expr.delete]/1: `delete []` followed by `(` is array delete unless the
// lookahead looks like a lambda (clang: `()`, or `(` and two identifiers).
// A parenthesized cast operand (`delete[] (char*)p`, libc++
// `deque/asan_turning_off.pass.cpp`) is an array delete of the cast.
//@ expect: run
struct S {
  void* b[2];
  S() {
    for (auto& p : b)
      p = new char[16];
  }
  ~S() {
    for (auto& p : b)
      delete[] (char*)p;
  }
};

int deleted = 0;
struct T {
  ~T() { ++deleted; }
};

int main() {
  { S s; }
  T* t = new T[3];
  void* v = t;
  delete[] (T*)v;
  delete[] (static_cast<T*>(nullptr));
  return deleted == 3 ? 0 : 1;
}
