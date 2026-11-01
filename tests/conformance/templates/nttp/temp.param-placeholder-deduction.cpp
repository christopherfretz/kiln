// [temp.param]/6.3, [temp.arg.nontype]/1: a non-type template parameter
// declared with `auto&`, `auto*`, `const auto&` or `decltype(auto)` deduces
// its type from the argument as a variable declaration would.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:5 error "non-type template parameter 'P' with type 'auto *' has incompatible initializer of type 'int'"
//@ diag: 15:17 note "template parameter is declared here"
//@ diag: 30:5 error "value of type 'int' is not implicitly convertible to 'int &'"
//@ diag: 32:25 error "no matching function for call to 'fp'"
//@ diag: 31:25 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'P'"
int gx = 1;
const int gc = 2;
int garr[3] = {4, 5, 6};
template <auto &R> struct Ref { static constexpr auto *p = &R; };
template <auto *P> struct Ptr { static constexpr auto *p = P; };
template <const auto &R> struct CRef { static constexpr auto *p = &R; };
template <decltype(auto) V> struct DA { using type = decltype(V); };
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
static_assert(Ref<gx>::p == &gx);
static_assert(Ref<gc>::p == &gc);
static_assert(same<decltype(Ref<gc>::p), const int *const>);
static_assert(Ptr<&gx>::p == &gx);
static_assert(Ptr<garr>::p == &garr[0]);
static_assert(CRef<gx>::p == &gx);
static_assert(same<DA<5>::type, int>);
static_assert(same<DA<(gx)>::type, int &>);
static_assert(same<DA<gc>::type, int>);
Ptr<5> bad1;
Ref<5> bad2;
template <auto *P> void fp() {}
void use() { fp<&gx>(); fp<1>(); }
