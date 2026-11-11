// C11 6.4.2.1, 6.10.3.2: in preprocessed output clang neither treats a
// Unicode space as whitespace nor drops a character that cannot start an
// identifier: each is an "other" token, printed as written.
//@ std: c11
//@ expect: pp
//@ pp-compare: text
a b\u00A0c
x ☃y ̀z
#define S(x) #x
S(a ☃x) S(p q)
q×r \u0300s
