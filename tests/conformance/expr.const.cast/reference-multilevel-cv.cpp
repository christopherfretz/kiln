// [expr.const.cast]/4: an lvalue of type T1 can be cast to "lvalue
// reference to T2" when a pointer to T1 can be const_cast to a pointer to
// T2, so cv-qualification may change below the top level too
// (bslstl_vector's `const_cast<const iterator&>(position)` with
// `iterator = J*` and a `const_iterator = const J*` position).
//@ expect: run
//@ exit: 7
struct J { int v; };
typedef J *iterator;
typedef const J *const_iterator;
int f(const_iterator p) {
  const iterator &r = const_cast<const iterator &>(p);
  r->v += 2;
  int *const *q = 0;
  const int *const *&qq = const_cast<const int *const *&>(q);
  return r->v + (qq == 0);
}
int main() { J j{4}; return f(&j); }
