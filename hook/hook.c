/*
 * 相关环境变量定义:
 * 1. C2RUST_PROJECT_ROOT: 工程的根目录，必须存在.
 * 2. C2RUST_FEATURE_ROOT: 构建的每个target都对应一个Feature, 必须存在
 * 3. C2RUST_CC: 编译程序的名字，如果不指定，则为gcc/clang/cc之一.
*/

#define _GNU_SOURCE
#include <errno.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/file.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#define MAX_PATH_LEN 8192
#define MAX_CMD_LEN 16384

static const char* C2RUST_PROJECT_ROOT = "C2RUST_PROJECT_ROOT";
static const char* C2RUST_FEATURE_ROOT = "C2RUST_FEATURE_ROOT";
static cosnt char* C2RUST_CC = "C2RUST_CC";

static const char* cc_names[] = {"gcc", "clang", "cc"};

static int is_compiler(const char* name) {
        const char* cc = getenv(C2RUST_CC);
        if (cc) {
                return strcmp(cc, name) == 0;
        }
        for (int i = 0; i < sizeof(cc_names) / sizeof(cc_names[0]); ++i) {
                if (strcmp(name, cc_names[i]) == 0) {
                        return 1;
                }
        }
        return 0;
}

char* path_from(const char* env) {
        const char* path = getenv(env);
        if (!path) {
                return 0;
        }
        return realpath(path, 0);
}

int is_cfile(const char* file) {
        int len = strlen(file);
        return len > 2 && strcmp(&file[len - 2], ".c") == 0;
}

// 提取-I, -D, -U, -include参数, 和工程目录下的C文件.
// 输入保证extracted, cfiles最少可以保存argc个输入参数.
static int parse_args(int argc, char* argv[], char* extracted[], char* cfiles[]) {
    int cnt = 0;
    for (int i = 0; i < argc; ++i) {
        char* arg = argv[i];
        if (arg[0] != '-') {
                if (is_cfile(arg) && access(arg, R_OK) == 0) {
                    *cfiles = realpath(arg, 0);
                    ++cfiles;
                }
                continue;
        }
        if (arg[1] == 'I' || arg[1] == 'D' || arg[1] == 'U') {
                if (arg[2]) {
                        extracted[cnt++] = arg;
                } else {
                        extracted[cnt++] = arg;
                        ++i;
                        if (i < argc) {
                                extracted[cnt++] = argv[i];
                        }
                }
        } else if (strcmp(&arg[1], "include") == 0) {
                extracted[cnt++] = arg;
                ++i;
                if (i < argc) {
                    extracted[cnt++] = arg;
                }
        }
    }

    return cnt;
}

static const char* strip_prefix(const char* path, const char* prefix) {
        int path_len = strlen(path);
        int prefix_len = strlen(prefix);
        if (strncmp(path, prefix, prefix_len)) {
               return 0;
        } else if (prefix[prefix_len - 1] == '/') {
                return &path[prefix_len];
        } else if (path[prefix_len] == '/') {
                return &path[prefix_len + 1];
        } else {
                return 0;
        }
}

static void preprocess_cfile(int argc, char* argv[], const char* cfile, const char* project_root, const char* feature_root) {
        const char* path = strip_prefix(cfile, project_root); 
        if (!path) return;

        // 获取预处理文件名, 后缀从.c修改为.c2rust
        char full_path[MAX_PATH_LEN];
        int full_path_len = snprintf(full_path, sizeof(full_path), "%s/c/%s2rust", feature_root, path);
        if (full_path_len >= sizeof(full_path)) return;

        char* filename = strrchr(full_path, '/');
        if (!filename) return; //绝对路径一定存在.

        // 创建预处理后文件存储路径
        *filename = 0; //忽略文件名
        char cmd[MAX_CMD_LEN];
        int cmd_len = snprintf(cmd, sizeof(cmd), "mkdir -p \"%s\"", full_path);
        if (cmd_len >= sizeof(cmd)) return;
        system(cmd);
        *filename = '/'; //恢复文件名.

        // 预处理命令
        cmd_len = snprintf(cmd, sizeof(cmd), "clang -E \"%s\" -o \"%s\"", cfile, full_path);
        if (cmd_len >= sizeof(cmd)) return;

        for (int i = 0; i < argc; ++i) {
                cmd_len += snprintf(cmd + cmd_len, sizeof(cmd) - cmd_len, " \"%s\"", argv[i]);
                if (cmd_len >= sizeof(cmd)) return;
        }
        system(cmd);
}

__attribute__((constructor)) static void c2rust_hook(int argc, char* argv[]) {
        if (!is_compiler(program_invocation_short_name)) {
                return;
        }

        char* project_root = 0;
        char* feature_root = 0;
        char* cflags[argc]; // 保存-I, -D, -U, -include
        char* cfiles[argc]; // 保存当前编译的C文件.

        memset(cflags, 0, sizeof(char*) * argc);
        memset(cfiles, 0, sizeof(char*) * argc);

        project_root = path_from(C2RUST_PROJECT_ROOT);
        if (!project_root) {
                return;
        }

        feature_root = path_from(C2RUST_FEATURE_ROOT);
        if (!feature_root) {
                goto fail;
        }
        
        unsetenv(C2RUST_PROJECT_ROOT);

        int cnt = parse_args(argc, argv, cflags, cfiles);
        if (!cfiles[0]) {
                goto fail;
        }

        for (int i = 0; i < argc; ++i) {
                const char* file = cfiles[i];
                if (!file) break;
                preprocess_cfile(cnt, cflags, file, project_root, feature_root);
        }
fail:
        if (project_root) free(project_root);
        if (feature_root) free(feature_root);
        for (char** cfile = cfiles; *cfile; ++cfile) {
                free(*cfile);
        }
}

