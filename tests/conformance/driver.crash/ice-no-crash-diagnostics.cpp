// -fno-crash-diagnostics: the banner is printed, no reproducer is written,
// and the exit code is 70 (clang 19 returns the crashed cc1's status
// unchanged; 70 is its crash code).
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -fkiln-crash-for-testing=parse -fno-crash-diagnostics
//@ ice: internal compiler error: crash requested by -fkiln-crash-for-testing=parse
//@ ice: ice-no-crash-diagnostics.cpp:11:1)
//@ ice: please submit a bug report

int x;
