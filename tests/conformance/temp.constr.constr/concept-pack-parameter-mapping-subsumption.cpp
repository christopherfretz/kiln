// [temp.constr.normal]/1.4, [temp.arg.general]/4: a concept-id's
// arguments for a trailing pack parameter form one pack in the parameter
// mapping, so atoms reached through different routes are identical and
// subsumption orders the overloads (libc++ relation.subsumption).
//@ expect: compile-ok
//@ strict-diags
template <class F, class... A> struct R { using type = int; };
template <class F, class... A> using res = typename R<F, A...>::type;
template <class T> concept bt = sizeof(T) > 0;
template <class F, class... A> concept pred = bt<res<F, A...>>;
template <class F, class T, class U>
concept rel = pred<F, T, T> && pred<F, U, U> && pred<F, T, U>;

template <class F, class T, class U> requires rel<F, T, T> && rel<F, U, U>
constexpr bool f() { return false; }
template <class F, class T, class U> requires rel<F, T, U>
constexpr bool f() { return true; }
static_assert(f<int, int, int>());

// A pack expansion inside the concept's alias is substituted element-wise.
template <class F, class... A> using call = decltype(F()(A()...));
template <class F, class... A> concept callable = bt<call<F, A...>>;
template <class F, class T> requires callable<F, T, T>
constexpr int g() { return 1; }
template <class F, class T> requires callable<F, T, T> && true
constexpr int g() { return 2; }
struct Fn { int operator()(int, int) const; };
static_assert(g<Fn, int>() == 2);
