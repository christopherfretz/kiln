// [temp.param]/7 (C++20): a class with an `_Atomic` non-static data member
// is not structural, so it cannot be the type of a non-type template
// parameter; a class with plain scalar members is. A parameter whose type
// was rejected is not compared again in a redeclaration (`AI`). Found by the
// clang negative-test run (temp.param/p7.cpp), fixed by WP-956.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:29 error "type 'Ext<_Atomic(float)>' of non-type template parameter is not a structural type"
//@ diag: 16:38 note "'Ext<_Atomic(float)>' is not a structural type because it has a non-static data member of non-structural type '_Atomic(float)'"
//@ diag: 18:27 error "type 'Ext<_Atomic(int)>' of non-type template parameter is not a structural type"
//@ diag: 16:38 note "'Ext<_Atomic(int)>' is not a structural type because it has a non-static data member of non-structural type '_Atomic(int)'"
//@ diag: 19:22 error "a non-type template parameter cannot have type '_Atomic(int)'"
//@ diag: 20:27 error "type 'Ext<_Atomic(int)>' of non-type template parameter is not a structural type"
//@ diag: 16:38 note "'Ext<_Atomic(int)>' is not a structural type because it has a non-static data member of non-structural type '_Atomic(int)'"
template <typename T> struct Ext { T t; };
template <Ext<_Atomic float>> struct AtomicFloat;
template <Ext<_Atomic int>> struct AtomicInt;
template <_Atomic int> struct AI;
template <Ext<_Atomic int>> struct AI;
template <Ext<float>> struct PlainFloat {};
template <Ext<int>> struct PlainInt {};
PlainInt<Ext<int>{1}> pi;
