// An internal compiler error during constant evaluation names the oven
// phase and the expression being evaluated.
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -fkiln-crash-for-testing=oven
//@ ice: internal compiler error: crash requested by -fkiln-crash-for-testing=oven
//@ ice: (phase: oven, at
//@ ice: ice-oven.cpp:12:
//@ ice: diagnostic msg:
//@ ice: diagnostic msg:

constexpr int two = 1 + 1;
