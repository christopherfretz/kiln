// [dcl.fct.def.default]/1 (C++17): an explicitly-defaulted special
// member cannot be variadic, whether defaulted on its first declaration
// or later: "an explicitly-defaulted copy constructor cannot be
// variadic". An assignment operator cannot be variadic at all
// ([over.oper]/8).
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:33 error "overloaded 'operator=' cannot be variadic"
//@ diag: 16:4 error "an explicitly-defaulted copy constructor cannot be variadic"
//@ diag: 17:7 error "overloaded 'operator=' cannot be variadic"
//@ diag: 18:12 error "an explicitly-defaulted copy constructor cannot be variadic"
//@ diag: 19:12 error "an explicitly-defaulted default constructor cannot be variadic"
//@ diag: 20:15 error "overloaded 'operator=' cannot be variadic"
struct S { S(const S&, ...); S& operator=(const S&, ...); S(); };
S::S(const S&, ...) = default;
S& S::operator=(const S&, ...) = default;
struct T { T(const T&, ...) = default; T(); };
struct U { U(...) = default; };
struct V { V& operator=(V&&, ...) = default; };
