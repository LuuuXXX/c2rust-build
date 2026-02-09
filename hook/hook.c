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
static const char* C2RUST_LD = "C2RUST_LD";
static const char* C2RUST_CC_SKIP = "C2RUST_CC_SKIP";
static const char* C2RUST_LD_SKIP = "C2RUST_LD_SKIP";

static const char* cc_names[] = {"gcc", "clang", "cc"};
static const char* ld_names[] = {"ld", "lld"};
static const char* ar_names[] = {"ar"};

static inline int is_matched(const char* name, const char** names, int len) {
        for (int i = 0; i < len; ++i) {
                if (strcmp(name, names[i]) == 0) {
                        return 1;
                }
        }
        return 0;
}

static inline int is_compiler(const char* name) {
        const char* cc = getenv(C2RUST_CC);
        if (!cc) {
            return is_matched(name, cc_names, sizeof(cc_names) / sizeof(cc_names[0]));
        } else {
            return strcmp(cc, name) == 0;
        }
}

static inline int is_linker(const char* name) {
        const char* ld = getenv(C2RUST_LD);
        if (!ld) {
            return is_matched(name, ld_names, sizeof(ld_names) / sizeof(ld_names[0]));
        } else {
            return strcmp(ld, name) == 0;
        }
}

static inline int is_archiver(const char* name) {
        return is_matched(name, ar_names, sizeof(ar_names) / sizeof(ar_names[0]));
}

static inline char* path_from(const char* env) {
        const char* path = getenv(env);
        if (!path) {
                return 0;
        }
        return realpath(path, 0);
}

static inline int is_cfile(const char* file) {
        int len = strlen(file);
        return len > 2 && strcmp(&file[len - 2], ".c") == 0;
}

// 提取-I, -D, -U, -include参数, 和工程目录下的C文件.
// 输入保证extracted, cfiles最少可以保存argc个输入参数.
static int parse_args(int argc, char* argv[], char* extracted[], char* cfiles[]) {
    int cnt = 0;
    for (int i = 1; i < argc; ++i) {
        char* arg = argv[i];
        if (arg[0] != '-') {
                if (is_cfile(arg) && access(arg, R_OK) == 0) {
                    *cfiles = realpath(arg, 0);
                    ++cfiles;
                }
                continue;
        }
        // 这里提取会影响预处理结果的所有参数
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
        } else if (strncmp(&arg[1], "std=", 4) == 0) {
                extracted[cnt++] = arg;
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
        char cmd[MAX_CMD_LEN];
        int cmd_len = snprintf(cmd, sizeof(cmd), "mkdir -p \"%s\"", full_path);
        if (cmd_len >= sizeof(cmd)) return;
        system(cmd);
        *filename = '/'; //恢复文件名.

        // 预处理命令, gcc和clang有差异, clang不能处理gcc生成的预处理文件, 后续bindgen都依赖clang，必须用clang生成
        // -P避免生成行号信息,混合构建时定位信息指向新生成的文件.
        cmd_len = snprintf(cmd, sizeof(cmd), "clang -E \"%s\" -o \"%s\" -P", cfile, full_path);
        if (cmd_len >= sizeof(cmd)) return;

        for (int i = 0; i < argc; ++i) {
                cmd_len += snprintf(cmd + cmd_len, sizeof(cmd) - cmd_len, " \"%s\"", argv[i]);
                if (cmd_len >= sizeof(cmd)) return;
        }
        system(cmd);
}

static void discover_cfile(int argc, char* argv[], const char* project_root, const char* feature_root) {
        char* cflags[argc]; // 保存-I, -D, -U, -include
        char* cfiles[argc]; // 保存当前编译的C文件.


        if (getenv(C2RUST_CC_SKIP)) return;

        memset(cflags, 0, sizeof(char*) * argc);
        memset(cfiles, 0, sizeof(char*) * argc);

        int cnt = parse_args(argc, argv, cflags, cfiles);
        if (!cfiles[0]) {
                goto fail;
        }

        setenv(C2RUST_CC_SKIP, "1", 0);

        for (int i = 0; i < argc; ++i) {
                const char* file = cfiles[i];
                if (!file) break;
                preprocess_cfile(cnt, cflags, file, project_root, feature_root);
        }
fail:
        for (char** cfile = cfiles; *cfile; ++cfile) {
                free(*cfile);
        }
}

// 提取生成的全部动态库和可执行程序的名字，以及生成过程中链接的C2RUST_PROJECT_ROOT目录下的静态库.
// 用户翻译的文件内容应该只包含在其中的一个库内，这样混合构建的时候，Rust的代码只作用于用户选择的库.
// 如果选择的是静态库，则Rust静态库总是和被选择的静态库一起使用.
// 如果选择的非静态库，则Rust静态库只在被选择的非静态库构建时使用.
// 这里提取的所有库都保存在C2RUST_FEATURE_ROOT/c/targets.list文件中，用户选择之后简单的将选择结果覆盖此文件即可.
static inline int ends_with(const char* str, const char* suffix) {
        int str_len = strlen(str);
        int suffix_len = strlen(suffix);
        if (str_len < suffix_len) return 0;
        return strcmp(str + str_len - suffix_len, suffix) == 0;
}

char* get_file(char* path) {
        char* deli = strrchr(path, '/');
        return deli ? deli + 1 : path;
}

char* get_static_lib(char* path, const char* project_root) {
        // 判断文件是否存在，如果存在是否在C2RUST_PROJECT_ROOT目录下.
        char* real_path = realpath(path, 0);
        if (!real_path) return 0;
        const char* tmp = strip_prefix(real_path, project_root);
        free(real_path);
        if (!tmp) return 0;
        
        // 提取文件名，判断是否是lib<...>.a
        char* lib = get_file(path);
        if (strncmp(lib, "lib", 3) != 0) return 0;
        int len = strlen(lib);
        if (len <= 5 || strcmp(&lib[len - 2], ".a") != 0) return 0;
        return lib;
}

static void target_save(char* libs[], int cnt, const char* feature_root) {
        if (cnt == 0) return;

        setenv(C2RUST_LD_SKIP, "1", 0);

        char buf[MAX_CMD_LEN];
        int len = snprintf(buf, MAX_CMD_LEN, "mkdir -p %s/c", feature_root);
        if (len >= MAX_CMD_LEN) {
                dprintf(2, "command is too long: %s...\n", buf);
                return;
        }
        system(buf);

        len = snprintf(buf, MAX_CMD_LEN, "%s/c/targets.list", feature_root);
        if (len >= MAX_CMD_LEN) {
                dprintf(2, "path is too long: %s...\n", buf);
                return;
        }

        int fd = open(buf, O_CREAT | O_RDWR, 0666);
        if (fd == -1) {
                dprintf(2, "failed to open file: %s...\n", buf);
                return;
        }

        if (flock(fd, LOCK_EX) != 0) {
                dprintf(2, "failed to lock file: %s, errno = %d\n", buf, errno);
                goto fail;
        }

        char* content = &buf[len + 1];
        ssize_t content_len = read(fd, content, MAX_CMD_LEN - len - 1);
        if (content_len == -1) {
                dprintf(2, "failed to read file: %s, errno = %d\n", buf, errno);
                goto fail;
        }
        // Null-terminate the content
        if (content_len > 0) {
            content[content_len] = 0; // Null terminate after the last byte read
        } else {
            content[0] = 0; // Empty file, null terminate at start
        }

        off_t off = lseek(fd, 0, SEEK_END);
        if (off == -1) {
                dprintf(2, "failed to access file: %s, errno = %d\n", buf, errno);
                goto fail;
        }

        for (int i = 0; i < cnt; ++i) {
            if (content_len == 0 || !strstr(content, libs[i])) {
                dprintf(fd, "%s\n", libs[i]);
            }
        }
fail:
        close(fd);
}

// Helper to check if string is an ar flag (like 'r', 'rcs', 'rv', etc.)
static inline int is_ar_flag(const char* arg) {
        // Starts with '-' is definitely a flag
        if (arg[0] == '-') return 1;
        
        // Check if it's a combination of ar operation/modifier letters
        // Common ar flags: r, c, s, t, u, v, d, x, p, q, m, a, b, i
        // They're typically combined like 'rcs', 'rv', 'crs', etc.
        size_t len = strlen(arg);
        if (len == 0 || len > 10) return 0; // ar flags are typically short
        
        for (size_t i = 0; i < len; ++i) {
                if (!strchr("rcstuvdxpqmabi", arg[i])) {
                        return 0; // Contains non-flag character
                }
        }
        return 1; // All characters are valid ar flag characters
}

// Discover targets from archiver (ar) commands
// ar command format: ar rcs libfoo.a file1.o file2.o ...
static void discover_archiver_target(int argc, char* argv[], const char* project_root, const char* feature_root) {
        if (getenv(C2RUST_LD_SKIP)) return;
        if (argc < 3) return; // Need at least: ar <flags> <archive>
        
        // The archive file is typically the first non-flag argument after the command
        for (int i = 1; i < argc; ++i) {
                char* arg = argv[i];
                
                // Skip flag arguments
                if (is_ar_flag(arg)) {
                        continue;
                }
                
                // Check if this is a .a file
                if (ends_with(arg, ".a")) {
                        char* lib = get_file(arg);
                        // Verify it matches lib*.a pattern
                        if (strncmp(lib, "lib", 3) == 0 && strlen(lib) > 5) {
                                char* libs[1] = {lib};
                                target_save(libs, 1, feature_root);
                        }
                        return; // Found the archive, done
                }
        }
}

static void discover_target(int argc, char* argv[], const char* project_root, const char* feature_root) {
        char* libs[argc];
        int pos = 0;

        if (getenv(C2RUST_LD_SKIP)) return;

        for (int i = 1; i < argc; ++i) {
                char* static_lib = get_static_lib(argv[i], project_root);
                if (static_lib) {
                        libs[pos++] = static_lib;
                } else if (strcmp(argv[i], "-o") == 0 && i < argc - 1) {
                        char* output = get_file(argv[i + 1]);
                        
                        // Filter out intermediate files and preprocessed files
                        // Keep: .so files, .a files, executables (no extension or not .o/.c2rust/.i)
                        int is_object = ends_with(output, ".o");
                        int is_preprocessed = ends_with(output, ".c2rust") || ends_with(output, ".i");
                        
                        if (!is_object && !is_preprocessed) {
                                libs[pos++] = output;
                        }
                }
        }
        target_save(libs, pos, feature_root);
}

__attribute__((constructor)) static void c2rust_hook(int argc, char* argv[]) {
        char* project_root = 0;
        char* feature_root = 0;
        project_root = path_from(C2RUST_PROJECT_ROOT);
        if (!project_root) {
                return;
        }

        feature_root = path_from(C2RUST_FEATURE_ROOT);
        if (!feature_root) {
                goto fail;
        }
        
        if (is_compiler(program_invocation_short_name)) {
               discover_cfile(argc, argv, project_root, feature_root);
               // Also track build targets when compiler is used for linking
               discover_target(argc, argv, project_root, feature_root);
        } else if (is_linker(program_invocation_short_name)) {
               discover_target(argc, argv, project_root, feature_root);
        } else if (is_archiver(program_invocation_short_name)) {
               discover_archiver_target(argc, argv, project_root, feature_root);
        }
fail:
        if (project_root) free(project_root);
        if (feature_root) free(feature_root);
}
