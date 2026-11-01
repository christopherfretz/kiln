// [over.match.class.deduct]/4, [over.match.list]/1.1: list-initialization
// tries the initializer-list guides first, with the whole list as their
// argument; not for a single element of a specialization of the template.
//@ strict-diags
namespace std { template<class E> class initializer_list { const E *b; decltype(sizeof 0) n; public: constexpr initializer_list() : b(nullptr), n(0) {} }; }
template<class T> struct V { V(std::initializer_list<T>); V(T, T); };
V v{1, 2, 3};
static_assert(__is_same(decltype(v), V<int>));
V w{1L, 2L};
static_assert(__is_same(decltype(w), V<long>));
V u = {1, 2};
template<class T> struct X { X(std::initializer_list<T>, int = 0); X(const X &); };
X x0{1L};
X x1{x0};
static_assert(__is_same(decltype(x1), X<long>));
X x2{x0, x0};
static_assert(__is_same(decltype(x2), X<X<long>>));
V vp(1, 2);
template<class T> struct D { D(); D(std::initializer_list<T>); };
D<int> di;
template<class T> struct E { E(std::initializer_list<T>); E(T); };
E e{1.0};
static_assert(__is_same(decltype(e), E<double>));
struct XD : X<long> { XD() : X<long>{1L} {} };
XD xd;
X x3{xd};
static_assert(__is_same(decltype(x3), X<long>));
X x4{xd, xd};
static_assert(__is_same(decltype(x4), X<XD>));
