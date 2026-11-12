// Mangling differential: vector types whose size or element type is
// dependent in a function template signature (Itanium ABI <vector-type>
// ::= Dv <number> _ <type> | Dv <dimension expression> _ <type>; clang
// `mangleType(const DependentVectorType *)` and
// `DependentSizedExtVectorType`). These used to mangle as `u7unknown`
// (libc++'s `__all_of(__simd_vector<_Tp, _Np>)` in `std::mismatch`, found
// by the hod backend project; WP-1355). `vector_size` takes a byte size,
// so a dependent element type keeps the written byte expression
// (`DvLi16E_T_`), while `ext_vector_type` with a constant count is a
// vector of the dependent element (`Dv4_T_`). The size expression's
// template parameter is a substitution candidate, as a dependent array
// bound's (`PS0_` is the second parameter's `T*`).
template <class T, unsigned long N> using ev = T __attribute__((ext_vector_type(N)));
template <class T, unsigned long N>
using vs = T __attribute__((__vector_size__(N * sizeof(T))));
template <class T> using ev4 = T __attribute__((ext_vector_type(4)));
template <class T> using vs16 = T __attribute__((__vector_size__(16)));
template <unsigned long N> using evi = int __attribute__((ext_vector_type(N)));
template <unsigned long N> using vsi = int __attribute__((__vector_size__(N)));

// Dependent size and element, dependent element only, dependent size only.
template <class T, unsigned long N> bool a1(ev<T, N>) { return true; }
template <class T, unsigned long N> bool a2(vs<T, N>) { return true; }
template <class T> bool a3(ev4<T>) { return true; }
template <class T> bool a4(vs16<T>) { return true; }
template <unsigned long N> bool a5(evi<N>) { return true; }
template <unsigned long N> bool a6(vsi<N>) { return true; }
// Substitutions: the vector type, its element, a qualified vector.
template <class T, unsigned long N> bool b1(ev<T, N>, ev<T, N> *, T) { return true; }
template <class T, unsigned long N>
bool b2(vs<T, N>, vs<T, N> *, const vs<T, N> &) { return true; }
template <class T, unsigned long N> ev<T, N> b3(ev<T, N> x) { return x; }
template <class T, unsigned long N> void b4(ev<T, N>, T *) {}
template <unsigned long N> void b5(evi<N>, evi<N> *) {}
// As template arguments.
template <class X> struct box {};
template <class T, unsigned long N> void c1(box<ev<T, N>>, box<vs<T, N>>) {}
template <class T> void c2(box<ev4<T>>, ev4<T>) {}
// The libc++ shape: a bool-returning helper on an alias template vector.
namespace n {
template <class T, unsigned long N> using simd = T __attribute__((__ext_vector_type__(N)));
template <class T, unsigned long N> bool all_of(simd<T, N> v) {
  return __builtin_reduce_and(v != 0);
}
} // namespace n

typedef int i4e __attribute__((ext_vector_type(4)));
typedef int i4v __attribute__((__vector_size__(16)));
typedef int i1e __attribute__((ext_vector_type(1)));
void use() {
  i4e e{};
  i4v v{};
  a1<int, 4>(e);
  a2<int, 4>(v);
  a3<int>(e);
  a4<int>(v);
  a5<4>(e);
  a6<16>(v);
  b1<int, 4>(e, &e, 1);
  b2<int, 4>(v, &v, v);
  b3<int, 4>(e);
  b4<int, 4>(e, nullptr);
  b5<4>(e, &e);
  c1<int, 4>({}, {});
  c2<int>({}, e);
  n::all_of<int, 1>(i1e{});
}
