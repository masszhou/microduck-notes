#define _GNU_SOURCE
#include <pthread.h>
#include <sched.h>
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
static void *check(void *unused) {
    (void)unused;
    pthread_attr_t attr;
    size_t size;
    assert(!pthread_getattr_np(pthread_self(), &attr));
    assert(!pthread_attr_getstacksize(&attr, &size));
    assert(size >= 2 * 1024 * 1024);
    assert(sched_getscheduler(0) == SCHED_OTHER);
    pthread_attr_destroy(&attr);
    return (void *)(intptr_t)123;
}
int main(void) {
    pthread_t thread;
    pthread_attr_t attr;
    struct sched_param param = {.sched_priority=20};
    assert(!pthread_attr_init(&attr));
    assert(!pthread_attr_setstacksize(&attr, 2 * 1024 * 1024));
    assert(!pthread_attr_setschedpolicy(&attr, SCHED_RR));
    assert(!pthread_attr_setschedparam(&attr, &param));
    assert(!pthread_attr_setinheritsched(&attr, PTHREAD_EXPLICIT_SCHED));
    assert(!pthread_create(&thread, &attr, check, NULL));
    void *result;
    assert(!pthread_join(thread, &result));
    assert((intptr_t)result == 123);
    pthread_attr_destroy(&attr);
    puts("PASS: denied RR(20) falls back, preserves stack size, executes and joins.");
    assert(!pthread_create(&thread, NULL, check, NULL));
    assert(!pthread_join(thread, &result));
    puts("PASS: default thread creation is unchanged.");
}
