// [cpp.pragma]/1, C23 7.12.2: malformed floating-point pragmas are
// diagnosed as clang does and dropped (clang's PragmaFPContractHandler,
// PragmaFPHandler, PragmaFloatControlHandler, PragmaSTDC_* handlers).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 29:26 warning "expected 'ON' or 'OFF' or 'DEFAULT' in pragma"
//@ diag: 30:25 warning "expected 'ON' or 'OFF' or 'DEFAULT' in pragma"
//@ diag: 31:30 warning "expected 'ON' or 'OFF' or 'DEFAULT' in pragma"
//@ diag: 32:26 warning "expected 'ON' or 'OFF' or 'DEFAULT' in pragma"
//@ diag: 33:17 error "missing option; expected 'contract', 'reassociate', 'recipro"
//@ diag: 34:18 error "invalid option 'bogus'; expected 'contract', 'reassociate',"
//@ diag: 35:26 error "expected '('"
//@ diag: 36:27 error "unexpected argument 'maybe' to '#pragma clang fp contract';"
//@ diag: 37:30 error "unexpected argument 'fast' to '#pragma clang fp reassociate'"
//@ diag: 38:29 error "unexpected argument 'on' to '#pragma clang fp exceptions'; e"
//@ diag: 39:30 error "unexpected argument 'float' to '#pragma clang fp eval_method"
//@ diag: 40:29 error "expected ')'"
//@ diag: 41:31 error "invalid option 'junk'; expected 'contract', 'reassociate', '"
//@ diag: 42:9 error "expected '('"
//@ diag: 44:32 error "pragma float_control is malformed; use 'float_control({push|"
//@ diag: 45:28 error "pragma float_control is malformed; use 'float_control({push|"
//@ diag: 46:42 warning "extra tokens at end of '#pragma float_control' - ignored"
//@ diag: 47:36 error "pragma float_control is malformed; use 'float_control({push|"
//@ diag: 48:24 warning "expected identifier in '#pragma FENV_ROUND' - ignored"
//@ diag: 49:25 warning "invalid or unsupported rounding mode in '#pragma STDC FENV_R"
//@ diag: 50:38 warning "extra tokens at end of '#pragma STDC FENV_ROUND' - ignored"
//@ diag: 51:37 warning "pragma STDC FENV_ROUND is not supported"
#pragma STDC FP_CONTRACT MAYBE
#pragma STDC FP_CONTRACT
#pragma STDC CX_LIMITED_RANGE
#pragma STDC FENV_ACCESS MAYBE
#pragma clang fp
#pragma clang fp bogus(on)
#pragma clang fp contract
#pragma clang fp contract(maybe)
#pragma clang fp reassociate(fast)
#pragma clang fp exceptions(on)
#pragma clang fp eval_method(float)
#pragma clang fp contract(on
#pragma clang fp contract(on) junk
#pragma float_control
#pragma float_control(precise)
#pragma float_control(precise, maybe)
#pragma float_control(bogus, on)
#pragma float_control(precise, on, push) junk
#pragma float_control(precise, on, pop)
#pragma STDC FENV_ROUND
#pragma STDC FENV_ROUND FE_SIDEWAYS
#pragma STDC FENV_ROUND FE_TONEAREST junk
#pragma STDC FENV_ROUND FE_TONEAREST
int x;
