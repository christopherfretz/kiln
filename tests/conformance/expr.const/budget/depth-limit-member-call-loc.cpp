// [expr.const]/10, [implimits]: the "exceeded maximum depth" note of a
// member call or an overloaded operator call is at the call's member name
// or operator (clang's CheckCallLimit at getExprLoc), its "in call to"
// notes at the call's first token; a call of a non-constexpr or undefined
// operator function is placed the same way (`a + b` at the `+`, `a[i]`,
// `a(i)`, `++a` at `a`). clang 19 and 22 alike (WP-1942).
//@ args: -fconstexpr-depth=3
//@ expect: compile-fail
//@ strict-diags
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:51 note "constexpr evaluation exceeded maximum depth of 3 calls"
//@ diag: 58:42 note "in call to 'R{this->n - 1}.get()'"
//@ diag: 58:42 note "in call to 'R{this->n - 1}.get()'"
//@ diag: 65:15 note "in call to 'r.get()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:51 note "constexpr evaluation exceeded maximum depth of 3 calls"
//@ diag: 58:42 note "in call to 'R{this->n - 1}.get()'"
//@ diag: 58:42 note "in call to 'R{this->n - 1}.get()'"
//@ diag: 66:15 note "in call to 'p->get()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:62 note "constexpr evaluation exceeded maximum depth of 3 calls"
//@ diag: 59:53 note "in call to 'R{this->n - 1}.operator+(1)'"
//@ diag: 59:53 note "in call to 'R{this->n - 1}.operator+(1)'"
//@ diag: 67:15 note "in call to 'r.operator+(1)'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:69 note "constexpr evaluation exceeded maximum depth of 3 calls"
//@ diag: 60:60 note "in call to 'R{this->n - 1}.operator==(r)'"
//@ diag: 60:60 note "in call to 'R{this->n - 1}.operator==(r)'"
//@ diag: 68:15 note "in call to 'r.operator==(r)'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:54 note "constexpr evaluation exceeded maximum depth of 3 calls"
//@ diag: 61:54 note "in call to 'R{this->n - 1}.operator[](1)'"
//@ diag: 61:54 note "in call to 'R{this->n - 1}.operator[](1)'"
//@ diag: 69:15 note "in call to 'r.operator[](1)'"
//@ diag: 82:15 error "static assertion expression is not an integral constant expression"
//@ diag: 82:17 note "non-constexpr function 'operator+' cannot be used in a constant expression"
//@ diag: 73:7 note "declared here"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 83:17 note "undefined function 'operator-' cannot be used in a constant expression"
//@ diag: 74:17 note "declared here"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 84:18 note "non-constexpr function 'operator+=' cannot be used in a constant expression"
//@ diag: 75:7 note "declared here"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:15 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 76:7 note "declared here"
//@ diag: 86:15 error "static assertion expression is not an integral constant expression"
//@ diag: 86:15 note "non-constexpr function 'operator++' cannot be used in a constant expression"
//@ diag: 77:7 note "declared here"
//@ diag: 87:15 error "static assertion expression is not an integral constant expression"
//@ diag: 87:17 note "non-constexpr function 'operator*' cannot be used in a constant expression"
//@ diag: 80:5 note "declared here"
//@ diag: 88:15 error "static assertion expression is not an integral constant expression"
//@ diag: 88:17 note "non-constexpr function 'get' cannot be used in a constant expression"
//@ diag: 78:7 note "declared here"
struct R {
  int n;
  constexpr int get() const { return n ? R{n - 1}.get() : 0; }
  constexpr int operator+(int k) const { return n ? R{n - 1} + k : k; }
  constexpr bool operator==(const R &o) const { return n ? R{n - 1} == o : true; }
  constexpr int operator[](int k) const { return n ? R{n - 1}[k] : k; }
};
constexpr R r{100};
constexpr const R *p = &r;
static_assert(r.get() == 0);
static_assert(p->get() == 0);
static_assert(r + 1 == 0);
static_assert(r == r);
static_assert(r[1] == 0);

struct Q {
  int n;
  int operator+(int k) const { return k; }
  constexpr int operator-(int k) const;
  int operator+=(int) const { return 0; }
  int operator()(int k) const { return k; }
  int operator++() const { return 0; }
  int get() const { return 0; }
};
int operator*(Q, int);
constexpr Q q{1};
static_assert(q + 1 == 0);
static_assert(q - 1 == 0);
static_assert((q += 1) == 0);
static_assert(q(1) == 0);
static_assert(++q == 0);
static_assert(q * 1 == 0);
static_assert(q.get() == 0);
