// [temp.inst]/14, [except.spec]/13: the noexcept-specifier of a class
// template specialization's destructor is instantiated when needed;
// `__is_nothrow_destructible` (and so libc++'s `destructible` /
// `default_initializable`) needs it and reads the instantiated value, not
// the implicit-destructor default (default_initializable.compile.pass.cpp,
// `NoexceptDependant<double>`).
//@ expect: compile-ok
//@ strict-diags
template <class T> struct N { ~N() noexcept(sizeof(T) == 0); };
template <class T> struct Y { ~Y() noexcept(sizeof(T) > 0); };
static_assert(!__is_nothrow_destructible(N<double>));
static_assert(__is_nothrow_destructible(Y<double>));
template <class T> concept nothrow_dtor = requires { requires __is_nothrow_destructible(T); };
static_assert(!nothrow_dtor<N<int>>);
static_assert(nothrow_dtor<Y<int>>);
