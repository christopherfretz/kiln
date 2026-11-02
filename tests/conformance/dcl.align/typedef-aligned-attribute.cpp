// [dcl.align], GNU `aligned` on a typedef: the typedef's type has the
// requested alignment (clang `getTypeInfo` of a `TypedefType`), including
// a member typedef of a class template whose alignment is instantiated,
// and `__atomic_is_lock_free` judges the object by that alignment (libc++
// `atomic_ref<T>::is_lock_free` uses `typedef _Tp _Aligned_Tp
// __attribute__((aligned(required_alignment)))`; is_always_lock_free.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
struct I2 { int i[2]; };
typedef I2 A8 __attribute__((aligned(8)));
static_assert(alignof(A8) == 8);
static_assert(alignof(I2) == 4);
constexpr bool f() { A8 a{}; return __atomic_is_lock_free(sizeof(I2), &a); }
static_assert(f());
template <class T> struct R {
  static constexpr unsigned long req = sizeof(T);
  typedef T Al __attribute__((aligned(req)));
  Al *p;
  constexpr bool lf() const { return __atomic_is_lock_free(sizeof(T), p); }
};
static_assert(alignof(R<I2>::Al) == 8);
constexpr bool h() { A8 a{}; R<I2> r{&a}; return r.lf(); }
static_assert(h());
