// When preprocessing itself crashes, the reproducer is the run script
// alone, and the banner says so.
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -fkiln-crash-for-testing=preprocess
//@ ice: internal compiler error: crash requested by -fkiln-crash-for-testing=preprocess
//@ ice: ice-preprocess.cpp' (phase: preprocess)
//@ ice: preprocessing the input also crashed; only the run script was written
//@ ice: .sh

#define X 1
int x = X;
