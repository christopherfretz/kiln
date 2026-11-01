//@ verify
// spec/harness.md "Directives" (`verify`): the expected-* comments match.
int x = undeclared; // expected-error {{use of undeclared identifier 'undeclared'}}
