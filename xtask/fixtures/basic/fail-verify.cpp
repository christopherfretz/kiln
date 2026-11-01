//@ verify
// spec/harness.md "Directives" (`verify`): an expected diagnostic that is
// not produced fails the -verify check.
int x = 0; // expected-error {{never emitted}}
