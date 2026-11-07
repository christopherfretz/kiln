// [expr.prim.lambda.general]/1: before C++23 an attribute-specifier-seq
// after the lambda-introducer (P2173R1) and a lambda-declarator without
// a parameter clause (P1102R2) are extensions: -Wc++23-lambda-attributes
// and -Wc++23-extensions. An attribute followed by the body needs no
// parameter clause.
//@ std: c++17
//@ strict-diags
//@ diag: 17:13 warning "an attribute specifier sequence in this position is a C++23 extension"
//@ diag: 18:13 warning "lambda without a parameter clause is a C++23 extension"
//@ diag: 19:13 warning "lambda without a parameter clause is a C++23 extension"
//@ diag: 20:13 warning "lambda without a parameter clause is a C++23 extension"
//@ diag: 21:13 warning "lambda without a parameter clause is a C++23 extension"
//@ diag: 22:13 warning "an attribute specifier sequence in this position is a C++23 extension"
//@ diag: 23:13 warning "lambda without a parameter clause is a C++23 extension"
//@ diag: 24:13 warning "an attribute specifier sequence in this position is a C++23 extension"
//@ diag: 24:27 warning "lambda without a parameter clause is a C++23 extension"
auto a = [] [[nodiscard]] () { return 1; };
auto b = [] mutable { return 1; };
auto c = [] -> int { return 1; };
auto d = [] noexcept { return 1; };
auto e = [] constexpr { return 1; };
auto f = [] [[nodiscard]] { return 1; };
auto j = [] __attribute__((noinline)) { return 1; };
auto k = [] [[nodiscard]] mutable { return 1; };
auto g = [] () [[]] { return 1; };
auto h = [] { return 1; };
