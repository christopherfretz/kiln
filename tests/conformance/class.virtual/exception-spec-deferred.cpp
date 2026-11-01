// [except.spec]/8, /13: an overrider's implicit exception specification (an
// implicit or defaulted special member, or a destructor without a
// noexcept-specifier) is computed before it is compared with the base's, and
// a destructor of a class template pattern is not checked (its implicit
// specification exists only in the instantiations). The libc++ shape: a
// facet's `~facet() override;` resolved by an earlier use, then a class
// template's `~num_get() override {}` (WP-171).
//@ expect: compile-ok
//@ strict-diags
struct B { virtual ~B(); virtual void f() noexcept; };
struct F : B { ~F() override; };
struct G : F { ~G() override {} };
G g;  // resolves F::~F to noexcept
template <class T> struct H : F { ~H() override {} };
H<int> h;
template <class T> struct I : F { T t; ~I() override = default; };
I<int> i;
template <class T> struct J : F { T t; };
J<int> j;
template <class T> struct K : B { void f() noexcept(T::v) override; };
struct Yes { static constexpr bool v = true; };
K<Yes> k;
struct L : F { ~L() override = default; };
struct N : B { struct Inner : F { int x; }; };
