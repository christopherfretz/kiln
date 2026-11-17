// [temp.spec.partial.general], [temp.expl.spec], [temp.arg.general]: a
// specialization whose template argument list holds an invalid argument
// (here an undeclared name in a decltype) is diagnosed once, at the
// argument, and the declaration is dropped: a second specialization with
// an equally invalid list is not a redefinition of the first (clang 22
// gives one error each), no specialization matches through it, and a
// later valid one is still declared and used. Class and variable
// templates, partial and explicit. Found running GCC's C++ testsuite.
// WP-1612.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:48 error "use of undeclared identifier 'f1'"
//@ diag: 26:48 error "use of undeclared identifier 'f2'"
//@ diag: 27:41 error "use of undeclared identifier 'f3'"
//@ diag: 28:41 error "use of undeclared identifier 'f4'"
//@ diag: 29:31 error "use of undeclared identifier 'g1'"
//@ diag: 30:31 error "use of undeclared identifier 'g2'"
//@ diag: 34:52 error "use of undeclared identifier 'f7'"
//@ diag: 35:52 error "use of undeclared identifier 'f8'"
//@ diag: 37:38 error "use of undeclared identifier 'g3'"
//@ diag: 38:38 error "use of undeclared identifier 'g4'"
template <class...> using void_t = void;
template <class T, class = void> struct A { static constexpr int k = 0; };
template <class T> struct A<T, void_t<decltype(f1())>> { static constexpr int k = 1; };
template <class T> struct A<T, void_t<decltype(f2())>> { static constexpr int k = 2; };
template <class T> struct A<T, decltype(f3())> {};
template <class T> struct A<T, decltype(f4())> {};
template <> struct A<decltype(g1())> {};
template <> struct A<decltype(g2())> {};
template <class T> struct A<T*, void> { static constexpr int k = 3; };
template <> struct A<int, void> { static constexpr int k = 4; };
template <class T> constexpr int v = 0;
template <class T> constexpr int v<void_t<decltype(f7()), T>> = 1;
template <class T> constexpr int v<void_t<decltype(f8()), T>> = 2;
template <class T> constexpr int v<T*> = 3;
template <> constexpr int v<decltype(g3())> = 5;
template <> constexpr int v<decltype(g4())> = 6;
template <> constexpr int v<int> = 4;
static_assert(A<long>::k == 0 && A<long*>::k == 3 && A<int>::k == 4);
static_assert(v<long> == 0 && v<long*> == 3 && v<int> == 4);
