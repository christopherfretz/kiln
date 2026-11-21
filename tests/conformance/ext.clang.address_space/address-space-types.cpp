// An address space is part of the type (WP-1803): a distinct type for
// template arguments, partial specialization and deduction, overloadable
// on, carried through typedefs, arrays (the elements are qualified) and
// member access, and invisible to classification and sizeof. The default
// space and address_space(0) are distinct types.
//@ expect: compile-ok
//@ strict-diags

struct S { int m; };
template <class A, class B> struct same { static constexpr bool v = false; };
template <class A> struct same<A, A> { static constexpr bool v = true; };

typedef __attribute__((address_space(1))) int AI;
static_assert(!same<AI, int>::v);
static_assert(!same<__attribute__((address_space(0))) int, int>::v);
static_assert(!same<__attribute__((address_space(1))) int, __attribute__((address_space(2))) int>::v);
static_assert(same<const AI, __attribute__((address_space(1))) const int>::v);
static_assert(same<const AI, const __attribute__((address_space(1))) int>::v);
typedef int [[clang::opencl_global]] OGI;
static_assert(same<__attribute__((opencl_global)) int, OGI>::v);
static_assert(!same<__attribute__((opencl_global)) int, __attribute__((opencl_global_device)) int>::v);
static_assert(sizeof(AI) == sizeof(int) && alignof(AI) == alignof(int));
static_assert(__is_integral(AI) && __is_arithmetic(const AI));
static_assert(__is_class(__attribute__((address_space(1))) S));

AI arr[3];
static_assert(same<decltype(arr[0]), AI &>::v);
static_assert(same<decltype(+arr[0]), int>::v);
static_assert(same<decltype(&arr[0]), AI *>::v);
int *__attribute__((address_space(1))) pg;
static_assert(same<decltype(pg), int *__attribute__((address_space(1)))>::v);
static_assert(same<decltype(*pg), int &>::v);

extern __attribute__((address_space(1))) S gs;
static_assert(same<decltype((gs.m)), AI &>::v);

template <class T> struct is_as1 { static constexpr bool v = false; };
template <class T> struct is_as1<__attribute__((address_space(1))) T> { static constexpr bool v = true; };
static_assert(is_as1<AI>::v);
static_assert(is_as1<__attribute__((address_space(1))) S>::v);
static_assert(!is_as1<int>::v);
static_assert(!is_as1<__attribute__((address_space(2))) int>::v);

template <class T> struct pointee;
template <class T> struct pointee<T *> { using type = T; };
static_assert(same<pointee<AI *>::type, AI>::v);
static_assert(same<pointee<const AI *>::type, const AI>::v);

template <class T> constexpr int deduce(T *) { return 0; }
template <class T> constexpr int deduce(__attribute__((address_space(1))) T *) { return 1; }
static_assert(deduce((int *)nullptr) == 0);
static_assert(deduce((AI *)nullptr) == 1);
static_assert(deduce((__attribute__((address_space(2))) int *)nullptr) == 0);

template <class T> constexpr bool id(T *) { return same<T, AI>::v; }
static_assert(id((AI *)nullptr));

constexpr int ov(int *) { return 0; }
constexpr int ov(AI *) { return 1; }
constexpr int ov(__attribute__((opencl_generic)) int *) { return 2; }
static_assert(ov((int *)nullptr) == 0);
static_assert(ov((AI *)nullptr) == 1);
static_assert(ov((__attribute__((opencl_generic)) int *)nullptr) == 2);
static_assert(ov((__attribute__((opencl_global)) int *)nullptr) == 2);

int redecl(AI *);
int redecl(int *);
int redecl(AI *p) { return *p; }

#if !__has_attribute(address_space) || !__has_attribute(opencl_global)
#error "address_space"
#endif
#if !__has_cpp_attribute(clang::address_space) || !__has_cpp_attribute(clang::opencl_generic)
#error "clang::address_space"
#endif
static_assert(__has_attribute(address_space) == 1);
