// __builtin_structured_binding_size(T) (clang 21+; WP-1740): the number
// of structured bindings an object of type T introduces
// ([dcl.struct.bind]/3-5), a size_t: an array's bound, a vector's or
// complex type's element count, tuple_size<T>::value for a tuple-like
// type, else the class's non-static data members (unnamed bit-fields
// excluded, the one class of the hierarchy that has members). Top-level
// cv is ignored; a type with no decomposition is a substitution failure.
// Oracle-checked by hand with clang++-22 (22.1.8): same answers.
//@ requires: clang>=22
//@ std: c++26
namespace std {
template <class T> struct tuple_size;
template <__SIZE_TYPE__ I, class T> struct tuple_element;
} // namespace std

struct A { int x; double y; };
struct B : A {};
struct E {};
struct BF { int a : 3; int : 4; int b; };
struct TL { int v; };
template <> struct std::tuple_size<TL> { static constexpr __SIZE_TYPE__ value = 5; };
typedef int V4 __attribute__((vector_size(16)));

static_assert(__builtin_structured_binding_size(A) == 2);
static_assert(__builtin_structured_binding_size(const volatile A) == 2);
static_assert(__builtin_structured_binding_size(B) == 2);
static_assert(__builtin_structured_binding_size(E) == 0);
static_assert(__builtin_structured_binding_size(BF) == 2);
static_assert(__builtin_structured_binding_size(int[7]) == 7);
static_assert(__builtin_structured_binding_size(const int[2][3]) == 2);
static_assert(__builtin_structured_binding_size(TL) == 5);
static_assert(__builtin_structured_binding_size(V4) == 4);
static_assert(__builtin_structured_binding_size(_Complex double) == 2);
static_assert(__is_same(decltype(__builtin_structured_binding_size(A)), __SIZE_TYPE__));

template <class T> constexpr auto count = __builtin_structured_binding_size(T);
static_assert(count<A> == 2 && count<int[3]> == 3);

template <class T>
concept Bindable = requires { __builtin_structured_binding_size(T); };
static_assert(Bindable<A> && Bindable<E> && Bindable<TL>);
static_assert(!Bindable<int> && !Bindable<A &> && !Bindable<int[]>);

template <class T, __SIZE_TYPE__ N = __builtin_structured_binding_size(T)>
constexpr __SIZE_TYPE__ size_of(int) { return N; }
template <class T> constexpr __SIZE_TYPE__ size_of(long) { return 99; }
static_assert(size_of<A>(0) == 2 && size_of<double>(0) == 99);

int arr[__builtin_structured_binding_size(A)];
static_assert(sizeof(arr) == 2 * sizeof(int));
