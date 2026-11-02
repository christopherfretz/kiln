// [dcl.align], GNU `aligned` on a typedef: a runtime
// `__atomic_is_lock_free` through a pointer to an aligned typedef of a
// power-of-two-sized type folds to true (clang emits no libatomic call;
// libc++ `atomic_ref<T>::is_lock_free`, is_always_lock_free.pass.cpp).
//@ expect: run
struct I2 { int i[2]; };
template <class T> struct R {
  static constexpr unsigned long req = sizeof(T);
  typedef T Al __attribute__((aligned(req)));
  Al *p;
  bool lf() const { return __atomic_is_lock_free(sizeof(T), p); }
};
int main() {
  alignas(8) I2 x{};
  R<I2> r{&x};
  return r.lf() ? 0 : 1;
}
