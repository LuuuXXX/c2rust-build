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
static const char* C2RUST_CC = "C2RUST_CC";

static const char* cc_names[] = {"gcc", "clang", "cc"};
static const char* ar_names[] = {"ar"};
static const char* ld_names[] = {"ld", "ld.gold", "ld.bfd"};

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

static int is_archiver(const char* name) {
        for (int i = 0; i < sizeof(ar_names) / sizeof(ar_names[0]); ++i) {
                if (strcmp(name, ar_names[i]) == 0) {
                        return 1;
                }
        }
        return 0;
}

static int is_linker(const char* name) {
        for (int i = 0; i < sizeof(ld_names) / sizeof(ld_names[0]); ++i) {
                if (strcmp(name, ld_names[i]) == 0) {
                        return 1;
                }
        }
        return 0;
}

// 检查是否是二进制文件（静态库.a、动态库.so或可执行文件）
static int is_binary_target(const char* filename) {
        if (!filename) return 0;
        
        int len = strlen(filename);
        // 检查是否是静态库 (.a)
        if (len > 2 && strcmp(&filename[len - 2], ".a") == 0) {
                return 1;
        }
        // 检查是否是动态库 (.so 或 .so.版本号)
        if (len > 3 && strcmp(&filename[len - 3], ".so") == 0) {
                return 1;
        }
        // 检查是否包含 .so. (比如 libfoo.so.1)
        if (strstr(filename, ".so.") != NULL) {
                return 1;
        }
        
        // 排除中间文件（.o, .c, .i, .c2rust 等）
        if (len > 2 && strcmp(&filename[len - 2], ".o") == 0) {
                return 0;
        }
        if (len > 2 && strcmp(&filename[len - 2], ".c") == 0) {
                return 0;
        }
        if (len > 2 && strcmp(&filename[len - 2], ".i") == 0) {
                return 0;
        }
        if (len > 7 && strcmp(&filename[len - 7], ".c2rust") == 0) {
                return 0;
        }
        
        // 如果没有扩展名，可能是可执行文件
        // 但需要确保不是 .o 文件或其他中间文件
        const char* dot = strrchr(filename, '.');
        const char* slash = strrchr(filename, '/');
        
        // 如果没有点，或者点在最后一个斜杠之前（即文件名本身没有扩展名）
        if (!dot || (slash && dot < slash)) {
                return 1; // 可能是可执行文件
        }
        
        return 0;
}

// 从文件路径中提取文件名（去除路径）
static const char* get_basename(const char* path) {
        const char* slash = strrchr(path, '/');
        return slash ? slash + 1 : path;
}

// 安全地创建目录（递归），不使用 system()
static int mkdir_p(const char* path) {
        char tmp[MAX_PATH_LEN];
        char *p = NULL;
        size_t len;
        
        len = snprintf(tmp, sizeof(tmp), "%s", path);
        if (len >= sizeof(tmp)) return -1;
        
        // 遍历路径，逐级创建目录
        for (p = tmp + 1; *p; p++) {
                if (*p == '/') {
                        *p = 0;
                        if (mkdir(tmp, 0755) != 0 && errno != EEXIST) {
                                return -1;
                        }
                        *p = '/';
                }
        }
        
        // 创建最后一级目录
        if (mkdir(tmp, 0755) != 0 && errno != EEXIST) {
                return -1;
        }
        
        return 0;
}

// 记录二进制目标到 targets.list
static void record_binary_target(const char* output_file, const char* feature_root) {
        if (!output_file || !feature_root) return;
        
        // 只记录文件名，不包含路径
        const char* basename = get_basename(output_file);
        
        if (!is_binary_target(basename)) {
                return;
        }
        
        // 构造 targets.list 文件路径
        char targets_list_path[MAX_PATH_LEN];
        int path_len = snprintf(targets_list_path, sizeof(targets_list_path), 
                               "%s/c/targets.list", feature_root);
        if (path_len >= sizeof(targets_list_path)) return;
        
        // 确保目录存在（安全方式，不使用 system()）
        char dir_path[MAX_PATH_LEN];
        int dir_len = snprintf(dir_path, sizeof(dir_path), "%s/c", feature_root);
        if (dir_len >= sizeof(dir_path)) return;
        
        if (mkdir_p(dir_path) != 0) {
                return; // 创建目录失败
        }
        
        // 以追加模式打开文件，带锁以支持并行构建
        int fd = open(targets_list_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
        if (fd < 0) return;
        
        // 获取文件锁
        if (flock(fd, LOCK_EX) < 0) {
                close(fd);
                return;
        }
        
        // 写入文件名和换行符
        dprintf(fd, "%s\n", basename);
        
        // 释放锁并关闭文件
        flock(fd, LOCK_UN);
        close(fd);
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

// 提取-I, -D, -U, -include参数, 和工程目录下的C文件, 以及输出文件(-o).
// 输入保证extracted, cfiles最少可以保存argc个输入参数.
static int parse_args(int argc, char* argv[], char* extracted[], char* cfiles[], char** output_file) {
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
        } else if (arg[1] == 'o') {
                // 提取输出文件名
                if (arg[2]) {
                        // -ofile 形式
                        *output_file = &arg[2];
                } else {
                        // -o file 形式
                        ++i;
                        if (i < argc) {
                                *output_file = argv[i];
                        }
                }
        }
    }

    return cnt;
}

static const char* strip_prefix(const char* path, const char* prefix) {
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
        if (mkdir_p(full_path) != 0) {
                return; // 创建目录失败
        }
        *filename = '/'; //恢复文件名.

        // 预处理命令
        char cmd[MAX_CMD_LEN];
        int cmd_len = snprintf(cmd, sizeof(cmd), "clang -E \"%s\" -o \"%s\"", cfile, full_path);
        if (cmd_len >= sizeof(cmd)) return;

        for (int i = 0; i < argc; ++i) {
                cmd_len += snprintf(cmd + cmd_len, sizeof(cmd) - cmd_len, " \"%s\"", argv[i]);
                if (cmd_len >= sizeof(cmd)) return;
        }
        system(cmd);
}

// 处理 archiver (ar) 调用，记录生成的静态库
__attribute__((constructor)) static void c2rust_archiver_hook(int argc, char* argv[]) {
        if (!is_archiver(program_invocation_short_name)) {
                return;
        }

        char* feature_root = path_from(C2RUST_FEATURE_ROOT);
        if (!feature_root) {
                return;
        }

        // ar 命令格式通常是: ar rcs libname.a file1.o file2.o ...
        // 或: ar -rcs libname.a file1.o file2.o ...
        // 第一个 .a 文件应该是输出文件
        for (int i = 1; i < argc; ++i) {
                char* arg = argv[i];
                int len = strlen(arg);
                
                // 查找第一个 .a 文件
                if (len > 2 && strcmp(&arg[len - 2], ".a") == 0) {
                        record_binary_target(arg, feature_root);
                        break;
                }
        }

        free(feature_root);
}

__attribute__((constructor)) static void c2rust_hook(int argc, char* argv[]) {
        if (!is_compiler(program_invocation_short_name)) {
                return;
        }

        char* project_root = 0;
        char* feature_root = 0;
        char* cflags[argc]; // 保存-I, -D, -U, -include
        char* cfiles[argc]; // 保存当前编译的C文件.
        char* output_file = 0; // 保存输出文件名

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

        int cnt = parse_args(argc, argv, cflags, cfiles, &output_file);
        
        // 记录二进制目标文件
        if (output_file) {
                record_binary_target(output_file, feature_root);
        }
        
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

