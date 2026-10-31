// [over.ref], [temp.deduct]: an `operator->` template whose parameters
// cannot be deduced is not viable; an ill-formed `operator->` is still
// found, so no `.` is suggested.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:22 error "no viable overloaded 'operator->'"
//@ diag: 15:25 note "candidate template ignored: couldn't infer template argument 'T'"
//@ diag: 19:6 error "overloaded 'operator->' must be a unary operator (has 2 parameters)"
//@ diag: 21:24 error "no viable overloaded 'operator->'"
//@ diag: 25:23 error "overload resolution selected deleted operator '->'"
//@ diag: 23:31 note "candidate function [with T = int] has been explicitly deleted"
struct P { int x = 3; };
struct Q {
  P p;
  template <class T> P* operator->();
};
int g(Q q) { return q->x; }
struct R2 {
  P* operator->(int);
};
int h2(R2 r) { return r->x; }
struct D {
  template <class T = int> P* operator->() = delete;
};
int h3(D d) { return d->x; }
