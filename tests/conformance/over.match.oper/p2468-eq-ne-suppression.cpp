// [over.match.oper]/4 (P2468): an `operator==` yields no reversed (or
// rewritten `!=`) candidate when a search for `operator!=` in the scope
// where it was found finds a corresponding function. A `!=` in another
// namespace, one with other parameter types, or a namespace-scope `!=`
// beside a member `==` does not suppress, nor does a hidden friend `!=`
// (the search does not find it); a namespace-scope `!=` beside a hidden
// friend `==` does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:24 error "invalid operands to binary expression ('int' and 'S')"
//@ diag: 25:6 note "candidate function not viable: no known conversion from 'int' to 'S' for 1st argument"
//@ diag: 50:24 error "invalid operands to binary expression ('int' and 'S')"
//@ diag: 48:25 note "candidate function template not viable: no known conversion from 'int' to 'S' for 1st argument"
//@ diag: 56:24 error "invalid operands to binary expression ('int' and 'S')"
//@ diag: 54:6 note "candidate function not viable: no known conversion from 'int' to 'S' for 1st argument"
//@ diag: 55:6 note "candidate function not viable: no known conversion from 'int' to 'S' for 1st argument"
//@ diag: 68:24 error "invalid operands to binary expression ('int' and 'S')"
//@ diag: 66:24 note "candidate function not viable: no known conversion from 'int' to 'S' for 1st argument"
namespace n1 { // a matching member `!=` suppresses the reversed `==`: no warning
struct S { bool operator==(const S&); bool operator!=(const S&); };
bool f(S s) { return s == s; }
}
namespace n2 { // a matching namespace-scope `!=`: `0 == s` has no candidate
struct S {};
bool operator==(S, int);
bool operator!=(S, int);
bool f(S s) { return 0 == s; }
}
namespace n3 { // a `!=` in another namespace does not suppress
struct S {};
bool operator==(S, int);
namespace m { bool operator!=(S, int); }
bool f(S s) { return 0 == s; }
}
namespace n4 { // a member `==` with a namespace-scope `!=` is not suppressed
struct S { bool operator==(int) const; };
bool operator!=(const S&, int);
bool f(S s) { return 0 == s; }
}
namespace n5 { // a `!=` with other parameter types does not correspond
struct S {};
bool operator==(S, int);
bool operator!=(S, long);
bool f(S s) { return 0 == s; }
}
namespace n6 { // corresponding templates
struct S {};
template <class T> bool operator==(S, T);
template <class T> bool operator!=(S, T);
bool f(S s) { return 0 == s; }
}
namespace n7 { // the rewritten `!(s == 0)` is suppressed too
struct S {};
bool operator==(S, int);
bool operator!=(S, int);
bool f(S s) { return 0 != s; }
}
namespace n8 { // a hidden friend `!=` is not found: `0 == s` is reversed
struct S {
  friend bool operator==(S, int) { return true; }
  friend bool operator!=(S, int) { return false; }
};
bool f(S s) { return 0 == s; }
}
namespace n9 { // a hidden friend `==` and a namespace-scope `!=`
struct S { friend bool operator==(S, int) { return true; } };
bool operator!=(S, int);
bool f(S s) { return 0 == s; }
}
