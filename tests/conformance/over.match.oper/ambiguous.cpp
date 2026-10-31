// [over.match.oper]/3, [over.match.best]/3: operator expressions whose
// user-declared candidates (or an enumeration's user-declared and built-in
// candidates) tie stay ambiguous, and every tied candidate is listed;
// [over.ics.ref]: a derived-class argument losing qualifiers is no
// known conversion.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:5 error "use of overloaded operator '+' is ambiguous (with operand types 'X' and 'int')"
//@ diag: 48:6 note "candidate function"
//@ diag: 49:6 note "candidate function"
//@ diag: 60:12 error "use of overloaded operator '==' is ambiguous (with operand types 'X' and 'X')"
//@ diag: 50:6 note "candidate function"
//@ diag: 51:6 note "candidate function"
//@ diag: 62:13 error "use of overloaded operator '-' is ambiguous (with operand types 'E' and 'int')"
//@ diag: 57:5 note "candidate function"
//@ diag: 62:13 note "built-in candidate operator-(int, int)"
//@ diag: 62:13 note "built-in candidate operator-(int, float)"
//@ diag: 62:13 note "built-in candidate operator-(int, double)"
//@ diag: 62:13 note "built-in candidate operator-(int, long double)"
//@ diag: 62:13 note "built-in candidate operator-(int, long)"
//@ diag: 62:13 note "built-in candidate operator-(int, long long)"
//@ diag: 62:13 note "built-in candidate operator-(int, __int128)"
//@ diag: 62:13 note "built-in candidate operator-(int, unsigned int)"
//@ diag: 62:13 note "built-in candidate operator-(int, unsigned long)"
//@ diag: 62:13 note "built-in candidate operator-(int, unsigned long long)"
//@ diag: 62:13 note "built-in candidate operator-(int, unsigned __int128)"
//@ diag: 62:13 note "built-in candidate operator-(float, int)"
//@ diag: 62:13 note "built-in candidate operator-(double, int)"
//@ diag: 62:13 note "built-in candidate operator-(long double, int)"
//@ diag: 62:13 note "built-in candidate operator-(long, int)"
//@ diag: 62:13 note "built-in candidate operator-(long long, int)"
//@ diag: 62:13 note "built-in candidate operator-(__int128, int)"
//@ diag: 62:13 note "built-in candidate operator-(unsigned int, int)"
//@ diag: 62:13 note "built-in candidate operator-(unsigned long, int)"
//@ diag: 62:13 note "built-in candidate operator-(unsigned long long, int)"
//@ diag: 62:13 note "built-in candidate operator-(unsigned __int128, int)"
//@ diag: 67:36 error "no matching function for call to 'f'"
//@ diag: 65:6 note "candidate function not viable: no known conversion from 'const DD' to 'B &' for 1st argument"
//@ diag: 65:18 note "candidate function not viable: no known conversion from 'const DD' to 'int' for 1st argument"
//@ diag: 58:6 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 67:42 error "no matching function for call to 'g'"
//@ diag: 66:6 note "candidate function not viable: no known conversion from 'const DD *' to 'B *' for 1st argument"
//@ diag: 66:18 note "candidate function not viable: no known conversion from 'const DD *' to 'int' for 1st argument"
struct B {}; struct DD : B {};
struct X {};
struct Y { Y(X); };
struct Z { Z(X); };
void operator+(Y, int);
void operator+(Z, int);
bool operator==(X, Y);
bool operator==(Y, X);
struct W { bool operator<(int) const; };
bool operator<(W, long);
enum E { e0 };
struct V { V(E); };
int operator-(V, int);
int operator-(E, long);
void f(X x, W w) {
  x + 1;
  (void)(x == x);
  (void)(w < 1);
  (void)(e0 - 1);
  (void)(e0 - 1L);
}
void f(B&); void f(int);
void g(B*); void g(int);
void k(const DD& d, const DD* p) { f(d); g(p); }
