// [WP-490] [temp.point]/1, [temp.inst]/7: a variable template
// specialization usable in constant expressions is instantiated at its
// reference and handed to codegen there (clang
// `InstantiateVariableDefinition`'s `PassToConsumerRAII`), before the
// namespace-scope declaration containing the reference: one with dynamic
// initialization gets its global, guard and `__cxx_global_var_init` before
// the function that names it, ordered among the TU's own dynamic
// initializations; a constant-initialized one is emitted at its first use
// (deferred, linkonce_odr). One that cannot be constant is instantiated at
// the end of the TU. A template declared before its definition is
// instantiated from the definition.
int dyn();
template <class T> const T k = T(dyn());
template <class T> const T k2 = T(dyn());
int f() { return k<int>; }
int a = dyn();
int g() { return k2<int> + k<long>; }
int b = dyn();

template <class T> constexpr T v = T(42);
template <class T> T w = T(7);
int h() { return v<int> + w<int>; }
const int* pv() { return &v<int>; }

template <class T> extern const int e;
template <class T> const int e = sizeof(T) * 3;
const int* pe() { return &e<short>; }
int c = dyn();
