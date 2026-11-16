// [class.derived.general]/2: a base class must be complete; the
// injected-class-name of a class still being defined names an incomplete
// class, also the injected-class-name of a class template used without
// arguments in its own definition (the current instantiation).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:23 error "base class has incomplete type"
//@ diag: 14:8 note "definition of 'D' is not complete until the closing '}'"
//@ diag: 15:42 error "base class has incomplete type"
//@ diag: 15:26 note "definition of 'TD<T>' is not complete until the closing '}'"
//@ diag: 17:15 error "base class has incomplete type"
//@ diag: 17:11 note "definition of 'X::Y' is not complete until the closing '}'"
struct D { struct Q : D { }; };
template<class T> struct TD { struct N : TD { }; };
struct X { struct Y; };
struct X::Y : X::Y { };
