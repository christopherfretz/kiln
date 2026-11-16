// [depr.static.constexpr] (C++17): the out-of-class redeclaration of a
// `constexpr` static data member defined in its class is redundant and
// deprecated. clang: -Wdeprecated-redundant-constexpr-static-def, in
// -Wdeprecated only (not -Wall or -Wextra); also for a member declared
// `inline` itself. Not a definition ([basic.def]/2), so a second one is
// no redefinition (WP-1517); attributes that change the definition's
// emission come after it and are ignored.
//@ verify
//@ std: c++17
//@ args: -Wdeprecated
struct A { static constexpr int n = 3; }; // expected-note 2 {{previous definition is here}}
constexpr int A::n; // expected-warning {{out-of-line definition of constexpr static data member is redundant in C++17 and is deprecated}}
constexpr int A::n; // expected-warning {{out-of-line definition of constexpr static data member is redundant in C++17 and is deprecated}}
[[gnu::used]] constexpr int A::n; // expected-warning {{attribute declaration must precede definition}} expected-warning {{redundant in C++17}}
__attribute__((weak)) constexpr int A::n; // expected-warning {{attribute declaration must precede definition}} expected-warning {{redundant in C++17}}
struct D { static inline constexpr int m = 4; };
constexpr int D::m; // expected-warning {{out-of-line definition of constexpr static data member is redundant in C++17 and is deprecated}}
struct C { static const int n = 5; };
const int C::n;
