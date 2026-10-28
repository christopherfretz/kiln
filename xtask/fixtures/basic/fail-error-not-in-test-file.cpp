//@ expect: compile-fail
//@ args: -DMISSING=1 -include does-not-exist.h
int ok;
