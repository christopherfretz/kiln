// [expr.prim.lambda.general]/3: an attribute-specifier-seq after the
// lambda-declarator appertains to the type of the call operator; one before
// it (C++23) to the call operator itself.
auto a = [] [[nodiscard]] () { return 1; };
auto b = [](int x) [[]] { return x; };
auto c = [] [[deprecated]] { return 2; };
void f() { (void)a(); (void)b(1); }
