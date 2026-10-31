#include <dlfcn.h>
static void *fixture_open(const char *path, int flags);
static void *fixture_symbol(void *library, const char *name);
static int fixture_close(void *library);
#define dlopen fixture_open
#define dlsym fixture_symbol
#define dlclose fixture_close
#define main context_fixture_main
#include "onnx_context.c"
#undef main

static int library_count, unavailable;
static void *fixture_open(const char *path, int flags) {
    (void)path; (void)flags;
    if (unavailable == 1) return NULL;
    library_count++; return (void *)1;
}
static void *fixture_symbol(void *library, const char *name) {
    (void)library; (void)name;
    return unavailable == 2 ? NULL : (void *)OrtGetApiBase;
}
static int fixture_close(void *library) {
    (void)library; library_count--; release_order = release_order * 10 + 4; return 0;
}
int main(void) {
    char error[128] = {0};
    PlainOnnxContext *context = plain_onnx_context("fixture", error, sizeof(error));
    if (!context || library_count != 1) return 1;
    PlainOnnxSession *session = plain_onnx_create(context, "fixture", 1, "CPU", error, sizeof(error));
    if (!session) return 2;
    plain_onnx_close(session);
    if (release_order != 12 || library_count != 1 || released) return 3;
    plain_onnx_close_context(context);
    if (release_order != 1234 || library_count || released != 1) return 4;
    release_order = 0;
    unavailable = 1;
    if (plain_onnx_context("fixture", error, sizeof(error)) || library_count || release_order) return 5;
    unavailable = 2;
    if (plain_onnx_context("fixture", error, sizeof(error)) || library_count || release_order != 4) return 6;
    release_order = 0;
    unavailable = 0;
    fail_telemetry = 1;
    if (plain_onnx_context("fixture", error, sizeof(error)) || library_count || release_order != 34 || released != 2) return 7;
    return 0;
}
