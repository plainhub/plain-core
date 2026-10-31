#include "../../../../native/inference/onnx_session.c"

static int environments, telemetry_calls, released, fail_telemetry, fail_session;
static int sessions, memories, options, release_order;
static OrtStatus *ORT_API_CALL create_env(OrtLoggingLevel level, const char *id, OrtEnv **out) {
    (void)level; (void)id; environments++; *out = (OrtEnv *)1; return NULL;
}
static OrtStatus *ORT_API_CALL disable_telemetry(const OrtEnv *env) {
    (void)env; telemetry_calls++; return fail_telemetry ? (OrtStatus *)1 : NULL;
}
static const char *ORT_API_CALL error_message(const OrtStatus *status) { (void)status; return "telemetry disable failed"; }
static void ORT_API_CALL release_status(OrtStatus *status) { (void)status; }
static void ORT_API_CALL release_env(OrtEnv *env) { (void)env; released++; release_order = release_order * 10 + 3; }
static OrtStatus *ORT_API_CALL create_options(OrtSessionOptions **out) { *out = (OrtSessionOptions *)1; options++; return NULL; }
static OrtStatus *ORT_API_CALL set_threads(OrtSessionOptions *options, int threads) { (void)options; (void)threads; return NULL; }
static OrtStatus *ORT_API_CALL set_optimization(OrtSessionOptions *options, GraphOptimizationLevel level) { (void)options; (void)level; return NULL; }
static OrtStatus *ORT_API_CALL disable_arena(OrtSessionOptions *options) { (void)options; return NULL; }
static OrtStatus *ORT_API_CALL create_memory(enum OrtAllocatorType type, enum OrtMemType memory_type, OrtMemoryInfo **out) {
    (void)type; (void)memory_type; memories++; *out = (OrtMemoryInfo *)1; return NULL;
}
static OrtStatus *ORT_API_CALL create_session(const OrtEnv *env, const ORTCHAR_T *path, const OrtSessionOptions *options, OrtSession **out) {
    (void)env; (void)path; (void)options;
    if (fail_session) return (OrtStatus *)1;
    sessions++; *out = (OrtSession *)1; return NULL;
}
static void ORT_API_CALL release_options(OrtSessionOptions *value) { (void)value; options--; }
static void ORT_API_CALL release_session(OrtSession *value) { (void)value; sessions--; release_order = release_order * 10 + 1; }
static void ORT_API_CALL release_memory(OrtMemoryInfo *value) { (void)value; memories--; release_order = release_order * 10 + 2; }
static const OrtApi api = {.CreateEnv = create_env, .DisableTelemetryEvents = disable_telemetry,
    .GetErrorMessage = error_message, .ReleaseStatus = release_status, .ReleaseEnv = release_env,
    .CreateSessionOptions = create_options, .SetIntraOpNumThreads = set_threads,
    .SetInterOpNumThreads = set_threads, .SetSessionGraphOptimizationLevel = set_optimization,
    .DisableCpuMemArena = disable_arena, .CreateCpuMemoryInfo = create_memory,
    .CreateSession = create_session, .ReleaseSessionOptions = release_options,
    .ReleaseSession = release_session, .ReleaseMemoryInfo = release_memory};
static const OrtApi *ORT_API_CALL get_api(uint32_t version) { return version == ORT_API_VERSION ? &api : NULL; }
static const char *ORT_API_CALL get_version(void) { return "fixture"; }
static const OrtApiBase base = {.GetApi = get_api, .GetVersionString = get_version};
const OrtApiBase *ORT_API_CALL OrtGetApiBase(void) { return &base; }
int main(void) {
    char error[128] = {0};
    PlainOnnxContext *context = plain_onnx_context("fixture", error, sizeof(error));
    if (!context || environments != 1 || telemetry_calls != 1) return 1;
    PlainOnnxSession *session = plain_onnx_create(context, "fixture", 1, "CPU", error, sizeof(error));
    if (!session || sessions != 1 || memories != 1 || options || released) return 3;
    plain_onnx_close(session);
    if (sessions || memories || options || released || release_order != 12) return 4;
    fail_session = 1;
    session = plain_onnx_create(context, "fixture", 1, "CPU", error, sizeof(error));
    if (session || sessions || memories || options || released || release_order != 122) return 5;
    plain_onnx_close_context(context);
    if (released != 1 || release_order != 1223) return 6;
    fail_telemetry = 1;
    context = plain_onnx_context("fixture", error, sizeof(error));
    if (context || environments != 2 || telemetry_calls != 2 || released != 2) return 2;
    return strcmp(error, "telemetry disable failed") != 0;
}
