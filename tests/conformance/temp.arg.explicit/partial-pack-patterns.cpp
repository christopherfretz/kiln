// [temp.arg.explicit]/9, [temp.deduct.type]/5.7: explicitly given packs
// in multi-pack patterns and in non-trailing function parameter packs.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:12 error "no matching function for call to 'k'"
//@ diag: 23:15 note "candidate template ignored: substitution failure [with T = <int>, U = <long, short>]: pack expansion contains parameter packs 'T' and 'U' that have different lengths (1 vs. 2)"
//@ diag: 29:11 error "no matching function for call to 'k2'"
//@ diag: 27:38 note "candidate template ignored: deduced type 'pair<char, [...]>' of 2nd parameter does not match adjusted type 'pair<long, [...]>' of argument [with T = <int, char>, U = <long, short>]"
//@ diag: 30:12 error "no matching function for call to 'k2'"
//@ diag: 27:38 note "candidate template ignored: substitution failure [with T = <int, char, int>, U = <long, short>]: pack expansion contains parameter packs 'T' and 'U' that have different lengths (3 vs. 2)"
//@ diag: 40:9 error "no matching function for call to 'm'"
//@ diag: 34:36 note "candidate function [with T = <char>] not viable: requires 2 arguments, but 3 were provided"
//@ diag: 41:9 error "no matching function for call to 'm'"
//@ diag: 34:36 note "candidate function [with T = <>] not viable: requires 1 argument, but 2 were provided"
//@ diag: 42:9 error "no matching function for call to 'm'"
//@ diag: 34:36 note "candidate function [with T = <char, long>] not viable: requires 3 arguments, but 2 were provided"
template<class A, class B> struct pair {};

// An explicit pack `T` is extended only when it alone forms the trailing
// pattern; with `U` in it, `U` is deduced from every element and `T`
// keeps its explicit elements.
template<class... T, class... U>
constexpr int k(pair<T, U>...) { return sizeof...(T) * 10 + sizeof...(U); }
static_assert(k<int, char>(pair<int, long>{}, pair<char, short>{}) == 22);
static_assert(k(pair<int, long>{}, pair<char, short>{}, pair<char, short>{}) == 33);

template<class... T, class... U> int k2(pair<T, U>...);
int bad0 = k<int>(pair<int, long>{}, pair<char, short>{});
int bad = k2<int, char>(pair<int, long>{}, pair<long, short>{});
int bad2 = k2<int, char, int>(pair<int, long>{}, pair<char, short>{});

// A non-trailing function parameter pack is a non-deduced context: its
// explicit elements are all of it.
template<class... T> constexpr int m(T..., int) { return sizeof...(T); }
static_assert(m<char, long>('a', 1L, 2) == 2);
static_assert(m<>(2) == 0);
static_assert(m(2) == 0);
template<class... T> constexpr int m2(T..., int, T...) { return sizeof...(T) * 100; }
static_assert(m2<char>('a', 1, 'b') == 100);
int a = m<char>('a', 1L, 2);
int b = m('a', 2);
int c = m<char, long>('a', 1L);
