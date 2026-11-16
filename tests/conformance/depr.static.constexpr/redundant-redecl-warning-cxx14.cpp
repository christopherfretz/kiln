// [class.static.data]/3 in C++14: the out-of-class declaration of a
// `constexpr` static data member is its definition, neither redundant
// nor deprecated: clang gives no -Wdeprecated-redundant-constexpr-static-def
// warning before C++17 (WP-1517; kiln warned in every mode), and a
// second definition is a redefinition.
//@ verify
//@ std: c++14
//@ args: -Wdeprecated
struct A { static constexpr int n = 3; };
constexpr int A::n; // expected-note {{previous definition is here}}
constexpr int A::n; // expected-error {{redefinition of 'n'}}
struct D { static constexpr int m = 4; };
[[gnu::used]] alignas(16) constexpr int D::m;
