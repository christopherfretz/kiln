// [implimits]/2 (nesting of parenthesized expressions), spec/sema-templates.md
// §1 P4, §10.3: 50000 nested parentheses exhaust the parser's stack; the
// stack guard stops with a fatal diagnostic instead of overflowing. The
// small -fkiln-stack-size makes the trip independent of the build profile.
//@ expect: compile-fail
//@ skip(clang): clang stops at its bracket depth (256) instead
//@ args: -fkiln-stack-size=8M
//@ diag: 22 fatal "kiln: stack exhausted during parsing"
#define O10 ((((((((((
#define C10 ))))))))))
#define O100 O10 O10 O10 O10 O10 O10 O10 O10 O10 O10
#define C100 C10 C10 C10 C10 C10 C10 C10 C10 C10 C10
#define O1000 O100 O100 O100 O100 O100 O100 O100 O100 O100 O100
#define C1000 C100 C100 C100 C100 C100 C100 C100 C100 C100 C100
#define O10000 O1000 O1000 O1000 O1000 O1000 O1000 O1000 O1000 O1000 O1000
#define C10000 C1000 C1000 C1000 C1000 C1000 C1000 C1000 C1000 C1000 C1000
#define O50000 O10000 O10000 O10000 O10000 O10000
#define C50000 C10000 C10000 C10000 C10000 C10000

int before = 1;

int x = O50000 0 C50000;

int after = 2;
