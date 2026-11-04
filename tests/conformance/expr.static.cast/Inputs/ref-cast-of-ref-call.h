// The functions of ir-ref-cast-of-ref-call.cpp and its clang 19 sibling
// ir-ref-cast-of-ref-call-19.cpp (WP-632).
#pragma once
void sink(double);
void sinki(int);
void sinkl(long long);
double x;
const double& coeff();
const int& icoeff();
struct M { const double& coeff(int, int) const; operator const double&() const; };
struct O { O& operator<<(double); };

void static_ref() { sink(static_cast<const double&>(coeff())); }
void c_style() { sink((const double&)coeff()); }
void nested() { sink(static_cast<const double&>(static_cast<const double&>(coeff()))); }
void rvalue_ref() { sink(static_cast<const double&&>(coeff())); }
void const_cast_ref() { sink(const_cast<double&>(coeff())); }
void int_ref() { sinki(static_cast<const int&>(icoeff())); }
void member(O& s, const M& m) { s << static_cast<const double&>(m.coeff(1, 2)); }
void conversion(const M& m) { sink(static_cast<const double&>(m)); }
void cond(bool b, double& y) { sink(b ? static_cast<const double&>(coeff()) : y); }
double ret() { return static_cast<const double&>(coeff()); }
double arith() { return static_cast<const double&>(coeff()) + 1.0; }
bool to_bool() { return static_cast<const double&>(coeff()); }
void reinterpret_call() { sinkl(reinterpret_cast<const long long&>(coeff())); }
void reinterpret_var() { sinkl(reinterpret_cast<const long long&>(x)); }
// The same call beneath a glvalue comma ([expr.comma]/1) or the chosen
// operand of `__builtin_choose_expr`.
void comma() { sink((sinki(0), coeff())); }
void comma_cast() { sink((sinki(0), static_cast<const double&>(coeff()))); }
void cast_comma() { sink(static_cast<const double&>((sinki(0), coeff()))); }
void choose() { sink(__builtin_choose_expr(1, coeff(), x)); }

// Regression pins: shapes that compiled before the fix.
void plain() { sink(coeff()); }
void to_value() { sink(static_cast<double>(coeff())); }
void via_var() { double d = static_cast<const double&>(coeff()); sink(d); }
void named() { const double& r = coeff(); sink(static_cast<const double&>(r)); }
void global() { sink(static_cast<const double&>(x)); }
void bind() { const double& r = static_cast<const double&>(coeff()); sink(r); }
