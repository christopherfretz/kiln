// WP-819: the alignment of a builtin's pointer argument follows clang
// `CodeGenFunction::EmitPointerWithAlignment` (spec/codegen.md §3 "Pointer
// alignment"): `&lvalue` and `std::addressof`/`std::__addressof`/
// `__builtin_addressof` take the operand lvalue's alignment (member
// `alignas` included), array decay the array's, derived-to-base the base
// subobject's, a no-op cast keeps its operand's, an explicit cast of a
// non-declaration operand and anything else the pointee type's (an
// `aligned` typedef pointee included). Self-contained replicas of the
// libstdc++ `std::atomic<T>`/`atomic_ref` and libc++
// `__cxx_atomic_base_impl`/`atomic_ref` shapes, without padding (the real
// headers also call `__builtin_clear_padding`, which clang 19 lacks).
using size_t = decltype(sizeof 0);
inline void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T> constexpr T *__addressof(T &r) noexcept {
  return __builtin_addressof(r);
}
template <class T> constexpr T *addressof(T &r) noexcept {
  return __builtin_addressof(r);
}
}
struct T1 { char a; };
struct T2 { char a, b; };
struct T4 { short a, b; };
struct T8 { int a, b; };
struct T16 { long long a, b; };

// libstdc++ `std::atomic<_Tp>`: `alignas(sizeof(_Tp)) _Tp _M_i` for a
// power-of-two size, accessed through `std::__addressof(_M_i)`.
template <class T> struct gnu_atomic {
  static constexpr int min_align =
      (sizeof(T) & (sizeof(T) - 1)) || sizeof(T) > 16 ? 0 : sizeof(T);
  static constexpr int align = min_align > alignof(T) ? min_align : alignof(T);
  alignas(align) T _M_i;
  T load(int m) const noexcept {
    alignas(T) unsigned char buf[sizeof(T)];
    T *ptr = reinterpret_cast<T *>(buf);
    __atomic_load(std::__addressof(_M_i), ptr, m);
    return *ptr;
  }
  void store(T i, int m) noexcept {
    __atomic_store(std::__addressof(_M_i), std::__addressof(i), m);
  }
  T exchange(T i, int m) noexcept {
    alignas(T) unsigned char buf[sizeof(T)];
    T *ptr = reinterpret_cast<T *>(buf);
    __atomic_exchange(std::__addressof(_M_i), std::__addressof(i), ptr, m);
    return *ptr;
  }
  bool cas(T &e, T i, int s, int f) noexcept {
    return __atomic_compare_exchange(std::__addressof(_M_i),
                                     std::__addressof(e),
                                     std::__addressof(i), true, s, f);
  }
};
// libstdc++ `__atomic_ref<_Tp>`: a plain `_Tp*`.
template <class T> struct gnu_atomic_ref {
  T *_M_ptr;
  T load(int m) const noexcept {
    alignas(T) unsigned char buf[sizeof(T)];
    T *dest = reinterpret_cast<T *>(buf);
    __atomic_load(_M_ptr, dest, m);
    return *dest;
  }
};
// libc++ `__cxx_atomic_base_impl<_Tp>`: an `_Atomic(_Tp)` member and the
// `__c11_atomic_*` builtins through `std::addressof`.
template <class T> struct cxx_atomic {
  _Atomic(T) __a_value;
};
template <class T> T cxx_load(const cxx_atomic<T> *a, int m) {
  using ptr_type = _Atomic(T) *;
  return __c11_atomic_load(const_cast<ptr_type>(std::addressof(a->__a_value)), m);
}
template <class T> void cxx_store(cxx_atomic<T> *a, T v, int m) {
  __c11_atomic_store(std::addressof(a->__a_value), v, m);
}
// libc++ `atomic_ref<_Tp>`: a pointer to an `aligned(required_alignment)`
// typedef of `_Tp`.
template <class T> struct cxx_atomic_ref {
  static constexpr size_t required_alignment = sizeof(T);
  typedef T _Aligned_Tp __attribute__((aligned(required_alignment)));
  _Aligned_Tp *__ptr_;
  T load(int m) const noexcept {
    alignas(T) unsigned char buf[sizeof(T)];
    T *ret = reinterpret_cast<T *>(buf);
    __atomic_load(__ptr_, ret, m);
    return *ret;
  }
};

template <class T> T use_gnu(gnu_atomic<T> &a, T v) {
  T e = a.load(5);
  a.store(v, 3);
  a.cas(e, a.exchange(v, 4), 5, 2);
  return e;
}
template T1 use_gnu(gnu_atomic<T1> &, T1);
template T2 use_gnu(gnu_atomic<T2> &, T2);
template T4 use_gnu(gnu_atomic<T4> &, T4);
template T8 use_gnu(gnu_atomic<T8> &, T8);
template T16 use_gnu(gnu_atomic<T16> &, T16);
T8 use_gnu_ref(gnu_atomic_ref<T8> r) { return r.load(5); }
T8 use_cxx(cxx_atomic<T8> *a, T8 v) {
  cxx_store(a, v, 5);
  return cxx_load(a, 5);
}
T16 use_cxx16(cxx_atomic<T16> *a) { return cxx_load(a, 2); }
T8 use_cxx_ref(cxx_atomic_ref<T8> r) { return r.load(5); }

// An over-aligned member, plain and through an explicit cast.
struct O { char c; alignas(16) T8 m; };
O go;
long long over(O *p) {
  T8 t;
  __atomic_load(&go.m, &t, 5);
  __atomic_load(&p->m, &t, 5);
  return __atomic_load_n((long long *)&go.m, 5) +
         __atomic_load_n(reinterpret_cast<long long *>(std::addressof(p->m)), 5);
}
// Array decay, a member array, derived-to-base, and a pointer variable.
alignas(16) int arr[4];
struct alignas(16) D : T8 { int c; };
D gd;
int decay(int *p) {
  T8 t;
  __atomic_load(static_cast<T8 *>(&gd), &t, 5);
  return __atomic_load_n(arr, 5) + __atomic_fetch_add(arr + 1, 1, 5) +
         __atomic_load_n(p, 5) + t.a;
}
// memcpy-like builtins share the inference.
void copy(T8 *dst) {
  __builtin_memcpy(dst, std::addressof(go.m), sizeof(T8));
  __builtin_memset(&gd, 0, sizeof(T8));
}
// Placement new: the pointer's alignment only when it is a declaration's.
alignas(16) unsigned char raw[16];
struct alignas(16) Buf { unsigned char b[16]; };
void place(void *q, Buf *pb, Buf &rb, Buf **pp) {
  new (std::addressof(raw)) T8{1, 2};
  new (q) T8{3, 4};
  new (&pb->b) T8{5, 6};
  new (&rb) T8{7, 8};
  new (&*pb) T8{9, 10};
  new (&(*pp)[1]) T8{11, 12};
  new (pb->b + 0) T8{13, 14};
}
// A member of a reference's referent is a declaration's alignment too.
int ref_member(O &r) {
  T8 t;
  __atomic_load(&r.m, &t, 5);
  __atomic_load(std::addressof(r.m), &t, 5);
  return t.a;
}
// Pointer arithmetic: clang 22 aligns the result at the offset from the
// base pointer's alignment (clang 19 takes the pointee type's).
alignas(16) long long g8[4];
void arith(int i, Buf *pb) {
  long long v;
  __atomic_load(g8 + 1, &v, 5);
  __atomic_load(g8 + 2, &v, 5);
  __atomic_load(2 + g8, &v, 5);
  __atomic_load(g8 + i, &v, 5);
  __atomic_load(&g8[2] - 0, &v, 5);
  new (pb->b + 8) T8{1, 2};
  new (pb->b + i) T8{3, 4};
}
