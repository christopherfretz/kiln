// [expr.cond]/4-5: a glvalue conditional whose arms are a
// reference-returning call and a variable, used as a prvalue. Each arm is
// loaded in its own block and the phi merges the loaded values (clang
// `VisitAbstractConditionalOperator` visits the arms as scalars; a
// reference-returning call is `EmitLoadOfLValue`). WP-418.
int& g();
struct It {
  int* p;
  int& operator*() const { return *p; }
};
void f1(int* y, bool c, int t) { *y = c ? g() : t; }
void f2(int* y, bool c, int t) { *y = c ? t : g(); }
void f3(const It& y, bool c, int t) { *y = c ? *y : t; }
int f4(bool c, bool d, int t) { return c ? (d ? g() : t) : g(); }
int& f5(bool c, int t) { return c ? g() : t; }
