// GNU typeof (spec/sema.md §5.8): __typeof/__typeof__ name the type of an
// unevaluated expression as written (no reference for an lvalue, cv kept,
// an id-expression's expression type), or a type-id; the operand may be an
// unparenthesized unary-expression. __typeof_unqual/__typeof_unqual__ drop
// the cv-qualifiers. Dependent operands are substituted at instantiation.
// Probe: clang++-19 -std=c++26 -fsyntax-only accepts this file silently.
//@ expect: compile-ok
//@ strict-diags
template<class A, class B> struct same { static constexpr bool v = false; };
template<class A> struct same<A, A> { static constexpr bool v = true; };
int i; int& ri = i; const int ci = 0; volatile long vl;
int f(int);
struct B { int bf : 3; int m; static double s; void mf(); };
static_assert(same<__typeof(i), int>::v);
static_assert(same<__typeof__(ri), int>::v);
static_assert(same<__typeof(ci), const int>::v);
static_assert(same<__typeof(vl), volatile long>::v);
static_assert(same<__typeof(int&), int&>::v);
static_assert(same<__typeof(const char*), const char*>::v);
static_assert(same<__typeof(f), int(int)>::v);
static_assert(same<__typeof(f)*, int(*)(int)>::v);
static_assert(same<__typeof(B().bf), int>::v);
static_assert(same<__typeof(B::m), int>::v);
static_assert(same<__typeof(&B::m), int B::*>::v);
static_assert(same<__typeof(&B::mf), void (B::*)()>::v);
static_assert(same<__typeof(B::s), double>::v);
static_assert(same<__typeof(void())*, void(*)()>::v);
static_assert(same<__typeof i, int>::v);
static_assert(same<__typeof(i++), int>::v);
static_assert(same<__typeof((i)), int>::v);
static_assert(same<__typeof(1+2L), long>::v);
static_assert(same<__typeof("ab"), const char[3]>::v);
static_assert(sizeof(__typeof(B)) == sizeof(B));
// As clang, __typeof_unqual does not start a type-id in a template
// argument or a cast: it is named through typedefs here.
typedef __typeof_unqual__(ci) U1;
typedef __typeof_unqual(const volatile int) U2;
typedef __typeof_unqual__(const int*) U3;
typedef __typeof_unqual__(const int[2]) U4;
static_assert(same<U1, int>::v);
static_assert(same<U2, int>::v);
static_assert(same<U3, const int*>::v);
static_assert(same<U4, int[2]>::v);
template<class T> struct W {
  __typeof(T{}) a;
  __typeof(T) b;
  __typeof(a) c;
  static_assert(same<__typeof(T()), T()>::v);
  auto get() -> __typeof(a) { return a; }
};
W<long> w;
static_assert(same<__typeof(w.c), long>::v);
static_assert(same<__typeof(w.get()), long>::v);
template<class T> auto add(T a, T b) -> __typeof(a + b) { return a + b; }
static_assert(same<__typeof(add(1, 2)), int>::v);
static_assert(same<__typeof(add('a', 'b')), int>::v);
template<class T> auto unq(T& a) -> __typeof_unqual__(a);
static_assert(same<__typeof(unq(ci)), int>::v);
static_assert(same<__typeof(unq(vl)), long>::v);
auto lam = [](auto a) -> __typeof(a) { return a; };
static_assert(same<__typeof(lam(1.0)), double>::v);
__typeof(i) j = (__typeof(i))3;
int k = __typeof(i){4};
// Not a keyword outside the GNU dialects.
int typeof = 1;
