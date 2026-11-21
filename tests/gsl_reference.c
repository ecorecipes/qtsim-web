/* The reference values of tests/gsl.rs: FNV-1a hashes of draws from GSL as
   installed (gsl_rng_mt19937, gsl_ran_poisson, gsl_ran_gaussian,
   gsl_rng_uniform_int). Build and run:
       cc -O2 $(gsl-config --cflags) -o gsl_reference tests/gsl_reference.c $(gsl-config --libs) -lm
       ./gsl_reference
   (GPL-3.0-or-later, as the rest of this repository.) */
#include <stdio.h>
#include <stdint.h>
#include <gsl/gsl_rng.h>
#include <gsl/gsl_randist.h>
static uint64_t fnv(uint64_t h, uint64_t v){ for(int i=0;i<8;i++){ h ^= (v>>(8*i))&0xff; h *= 1099511628211ULL; } return h; }
int main(void){
  double mus[] = {0.3, 3.0, 9.9, 10.5, 14.0, 25.3, 50.0, 137.0, 1000.0, 1e5};
  for(int j=0;j<10;j++){ gsl_rng *r=gsl_rng_alloc(gsl_rng_mt19937); gsl_rng_set(r, 1000+j); uint64_t h=1469598103934665603ULL; for(int i=0;i<5000;i++) h=fnv(h, gsl_ran_poisson(r, mus[j])); printf("(%.1f, %llu, 0x%016llxu64),\n", mus[j], (unsigned long long)(1000+j), (unsigned long long)h); gsl_rng_free(r); }
  { gsl_rng *r=gsl_rng_alloc(gsl_rng_mt19937); gsl_rng_set(r, 77); uint64_t h=1469598103934665603ULL; union{double d; uint64_t u;} c; for(int i=0;i<100000;i++){ c.d=gsl_ran_gaussian(r, 0.37); h=fnv(h,c.u);} printf("gaussian 0x%016llx\n",(unsigned long long)h); }
  { gsl_rng *r=gsl_rng_alloc(gsl_rng_mt19937); gsl_rng_set(r, 0); uint64_t h=1469598103934665603ULL; for(int i=0;i<10000;i++) h=fnv(h, gsl_rng_uniform_int(r, 999)); printf("uniform_int seed0 0x%016llx\n",(unsigned long long)h); }
  return 0; }
