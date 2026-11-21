// Overload sets with a TU-local function in forms that do not name it in
// an exposure (exposure-overload-set-importer-run.cpp; the module-only
// checks are basic.link/exposure-overload-set-accepted.cpp).
export module xovs;
static int f(int) { return 1; }
int f(double) { return 2; }
namespace N {
export struct A {};
export int adl(A) { return 3; }
static int adl(int) { return 4; }
}
int adl(double) { return 5; }
export struct B { int v; };
static B operator+(B a, int) { return {a.v + 100}; }
export B operator+(B a, B b) { return {a.v + b.v}; }
template <class T> inline int calls_adl(T x) { return adl(x); }
template <class T> int not_inline(T) { return f(1.0); }

export inline B operator_expression() { return B{1} + B{2}; }
export int not_inline_body() { return f(1.0) + adl(N::A{}) + f(1) + N::adl(1); }
export int variable = f(1.0) + f(1);
export inline int instantiation() { return calls_adl(0.5); }
export inline int non_inline_template() { return not_inline(1); }
