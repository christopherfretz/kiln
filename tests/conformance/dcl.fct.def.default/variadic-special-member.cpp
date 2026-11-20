// [dcl.fct.def.default]/2.5, /2: an explicitly-defaulted special member
// cannot be variadic; since C++20 one whose type differs from the
// implicit declaration's is defined as deleted when defaulted on its
// first declaration (a -Wdefaulted-function-deleted warning), so only a
// later defaulting is an error. An assignment operator cannot be
// variadic at all ([over.oper.general]/8).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:33 error "overloaded 'operator=' cannot be variadic"
//@ diag: 19:4 error "an explicitly-defaulted copy constructor cannot be variadic"
//@ diag: 20:7 error "overloaded 'operator=' cannot be variadic"
//@ diag: 21:12 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 21:12 note "function is implicitly deleted because its declared type does not match the type of an implicit copy constructor"
//@ diag: 22:12 warning "explicitly defaulted default constructor is implicitly deleted"
//@ diag: 22:12 note "function is implicitly deleted because its declared type does not match the type of an implicit default constructor"
//@ diag: 23:15 error "overloaded 'operator=' cannot be variadic"
struct S { S(const S&, ...); S& operator=(const S&, ...); S(); };
S::S(const S&, ...) = default;
S& S::operator=(const S&, ...) = default;
struct T { T(const T&, ...) = default; T(); };
struct U { U(...) = default; };
struct V { V& operator=(V&&, ...) = default; };
