// GNU `vector_size` and clang `ext_vector_type` among the decl-specifiers
// of a type-id ([dcl.name]; clang processTypeAttrs, HandleVectorSizeAttr,
// HandleExtVectorTypeAttr): in `sizeof`/`alignof`, casts, template
// arguments, partial specialization patterns and new-type-ids the
// attribute modifies the type, including a dependent element or size
// substituted later (WP-187). `ext_vector_type` is a type attribute here,
// so clang 19 accepts it too.
//@ expect: run
template <class T> struct S { static constexpr int n = sizeof(T); };
template <class T, int N> constexpr int vs() { return sizeof(T __attribute__((vector_size(N * sizeof(T))))); }
template <class T, int N> constexpr int es() { return sizeof(T __attribute__((ext_vector_type(N)))); }
static_assert(sizeof(__attribute__((vector_size(16))) int) == 16);
static_assert(sizeof(int __attribute__((ext_vector_type(4)))) == 16);
static_assert(alignof(double __attribute__((vector_size(16)))) == 16);
static_assert(S<int __attribute__((vector_size(8)))>::n == 8);
static_assert(S<__attribute__((vector_size(8))) int>::n == 8);
static_assert(vs<short, 8>() == 16);
static_assert(es<float, 4>() == 16);
static_assert(es<char, 3>() == 4);
template <class T> constexpr bool is_vec = false;
template <class T> constexpr bool is_vec<T __attribute__((vector_size(16)))> = true;
static_assert(is_vec<int __attribute__((vector_size(16)))>);
static_assert(!is_vec<int>);
using T32 = int __attribute__((vector_size(32)));
static_assert(sizeof(T32) == 32);

int main() {
  auto v = (int __attribute__((vector_size(8))))(long long)0x0000000200000001;
  if (v[0] != 1 || v[1] != 2)
    return 1;
  // A scalar cast to an ext vector splats.
  auto e = (int __attribute__((ext_vector_type(2))))3;
  if (e.x != 3 || e.y != 3)
    return 2;
  auto *p = new (int __attribute__((ext_vector_type(4))))(7);
  int r = (*p).w;
  delete p;
  auto *q = new int __attribute__((vector_size(16)))[2]{};
  r += sizeof(q[0]);
  delete[] q;
  return r == 7 + 16 ? 0 : 3;
}
