// [except.spec]/8, [class.dtor]/5: a destructor without a
// noexcept-specifier is implicitly non-throwing when every member and
// base destructor is, so a potentially-throwing call in its body is an
// `invoke` that unwinds to `__clang_call_terminate` ([except.terminate]).
// This holds for an out-of-line destructor nothing in the TU uses (clang
// `ActOnStartOfFunctionDef` resolves the exception specification of
// every definition), and the call to it is `nounwind`.
void may_throw();
struct S { ~S(); };
S::~S() { may_throw(); }
struct T { ~T() { may_throw(); } };
void use(T* p) { p->~T(); }
struct U { ~U(); int u; };
U::~U() = default;
struct V { ~V(); U u; };
V::~V() { may_throw(); }
void use2(S* s) { s->~S(); }
