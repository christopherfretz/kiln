// [dcl.mptr]/1 with [temp.names]/4: a member-pointer type-id whose
// nested-name-specifier has template-id components (`E<int>::*`,
// `N::H<int>::*`, `::E<int>::*`, the dependent `B<I, T>::*`) is a template
// argument; the component's own `>` does not close the enclosing list
// (boost's `safe_bool<IteratorT iterator_range_base<IteratorT, Tag>::*>`).
//@ strict-diags
template <class P, class Q = int> struct sb { typedef P type; };
template <class T> struct E { typedef int X; };
namespace N { template <class T> struct H {}; }
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

sb<int E<int>::*> a;
sb<int N::H<int>::*, int> b;
typedef sb<int ::E<int>::*> T1;
static_assert(same<T1::type, int E<int>::*>);
static_assert(same<sb<E<int>::X E<int>::*>::type, int E<int>::*>);

template <class I, class T> class B {
  typedef sb<I B<I, T>::*> safe_bool_t;
public:
  typedef typename safe_bool_t::type ubt;
};
static_assert(same<B<int *, int>::ubt, int *B<int *, int>::*>);
void f() { sb<int E<int>::*> n; (void)n; }
