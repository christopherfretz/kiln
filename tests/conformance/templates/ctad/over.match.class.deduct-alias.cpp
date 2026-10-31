// [over.match.class.deduct]/3 (P1814): deduction through an alias template.
//@ xfail(kiln): alias template CTAD (P1814) is deferred past WP-071b
//@ strict-diags
template<class T, class U> struct P { P(T, U); };
template<class T> using Same = P<T, T>;
Same s(1, 2);
static_assert(__is_same(decltype(s), P<int, int>));
