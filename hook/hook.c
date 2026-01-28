#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dlfcn.h>
#include <sys/file.h>
#include <limits.h>
#include <spawn.h>

// Function pointer types
typedef int (*execve_func_t)(const char *pathname, char *const argv[], char *const envp[]);
typedef int (*execv_func_t)(const char *pathname, char *const argv[]);
typedef int (*execvp_func_t)(const char *file, char *const argv[]);
typedef int (*posix_spawn_func_t)(pid_t *pid, const char *path,
                                   const posix_spawn_file_actions_t *file_actions,
                                   const posix_spawnattr_t *attrp,
                                   char *const argv[], char *const envp[]);

// Original functions
static execve_func_t original_execve = NULL;
static execv_func_t original_execv = NULL;
static execvp_func_t original_execvp = NULL;
static posix_spawn_func_t original_posix_spawn = NULL;

// Initialize function pointers
__attribute__((constructor))
static void init_hooks(void) {
    original_execve = (execve_func_t)dlsym(RTLD_NEXT, "execve");
    original_execv = (execv_func_t)dlsym(RTLD_NEXT, "execv");
    original_execvp = (execvp_func_t)dlsym(RTLD_NEXT, "execvp");
    original_posix_spawn = (posix_spawn_func_t)dlsym(RTLD_NEXT, "posix_spawn");
}

// Check if a path ends with a given suffix
static int ends_with(const char *str, const char *suffix) {
    if (!str || !suffix) return 0;
    size_t str_len = strlen(str);
    size_t suffix_len = strlen(suffix);
    if (suffix_len > str_len) return 0;
    return strcmp(str + str_len - suffix_len, suffix) == 0;
}

// Check if a path is within the project root
static int is_in_project_root(const char *path, const char *project_root) {
    if (!path || !project_root) return 0;
    
    char abs_path[PATH_MAX];
    if (realpath(path, abs_path) == NULL) {
        return 0;
    }
    
    char abs_root[PATH_MAX];
    if (realpath(project_root, abs_root) == NULL) {
        return 0;
    }
    
    size_t root_len = strlen(abs_root);
    if (strncmp(abs_path, abs_root, root_len) != 0) {
        return 0;
    }
    
    char next = abs_path[root_len];
    return next == '\0' || next == '/';
}

// Extract the compiler name from a path
static const char* get_compiler_name(const char *pathname) {
    const char *name = strrchr(pathname, '/');
    return name ? name + 1 : pathname;
}

// Check if this is a compiler we should track
static int is_tracked_compiler(const char *pathname) {
    const char *name = get_compiler_name(pathname);
    return strcmp(name, "gcc") == 0 || 
           strcmp(name, "clang") == 0 || 
           strcmp(name, "cc") == 0;
}

// Check if the compilation involves a .c file
static int has_c_file(char *const argv[]) {
    for (int i = 0; argv[i] != NULL; i++) {
        if (ends_with(argv[i], ".c")) {
            return 1;
        }
    }
    return 0;
}

// Write compilation info to output file
static void log_compilation(const char *pathname, char *const argv[], char *const envp[]) {
    const char *output_file = getenv("C2RUST_OUTPUT_FILE");
    const char *project_root = getenv("C2RUST_ROOT");
    
    if (!output_file || !project_root) {
        return;
    }
    
    FILE *fp = fopen(output_file, "a");
    if (!fp) {
        return;
    }
    
    int fd = fileno(fp);
    flock(fd, LOCK_EX);
    
    char cwd[PATH_MAX];
    if (getcwd(cwd, sizeof(cwd)) == NULL) {
        flock(fd, LOCK_UN);
        fclose(fp);
        return;
    }
    
    fprintf(fp, "---ENTRY---\n");
    
    int first_option = 1;
    for (int i = 1; argv[i] != NULL; i++) {
        const char *arg = argv[i];
        
        if (arg[0] == '-') {
            if (strncmp(arg, "-I", 2) == 0 || 
                strncmp(arg, "-D", 2) == 0 || 
                strncmp(arg, "-U", 2) == 0 ||
                strncmp(arg, "-std", 4) == 0 ||
                strncmp(arg, "-include", 8) == 0) {
                
                if (!first_option) {
                    fprintf(fp, " ");
                }
                fprintf(fp, "%s", arg);
                first_option = 0;
                
                if ((strcmp(arg, "-I") == 0 || 
                     strcmp(arg, "-D") == 0 || 
                     strcmp(arg, "-U") == 0 ||
                     strcmp(arg, "-include") == 0) && argv[i + 1] != NULL) {
                    fprintf(fp, " %s", argv[i + 1]);
                    i++;
                }
            }
        } else if (ends_with(arg, ".c")) {
            char abs_file[PATH_MAX];
            if (arg[0] == '/') {
                strcpy(abs_file, arg);
            } else {
                snprintf(abs_file, PATH_MAX, "%s/%s", cwd, arg);
            }
            
            if (is_in_project_root(abs_file, project_root)) {
                fprintf(fp, "\n%s\n", abs_file);
                fprintf(fp, "%s\n", cwd);
            }
        }
    }
    
    flock(fd, LOCK_UN);
    fclose(fp);
}

// Intercept execve calls
int execve(const char *pathname, char *const argv[], char *const envp[]) {
    // Initialize if needed
    if (original_execve == NULL) {
        original_execve = (execve_func_t)dlsym(RTLD_NEXT, "execve");
    }
    
    // Check if this is a compiler we want to track
    if (is_tracked_compiler(pathname) && has_c_file(argv)) {
        log_compilation(pathname, argv, envp);
    }
    
    // Call the original execve
    return original_execve(pathname, argv, envp);
}

// Intercept execv calls
int execv(const char *pathname, char *const argv[]) {
    // Initialize if needed
    if (original_execv == NULL) {
        original_execv = (execv_func_t)dlsym(RTLD_NEXT, "execv");
    }
    
    // Check if this is a compiler we want to track
    if (is_tracked_compiler(pathname) && has_c_file(argv)) {
        // Get current environment for logging
        extern char **environ;
        log_compilation(pathname, argv, environ);
    }
    
    // Call the original execv
    return original_execv(pathname, argv);
}

// Intercept execvp calls  
int execvp(const char *file, char *const argv[]) {
    // Initialize if needed
    if (original_execvp == NULL) {
        original_execvp = (execvp_func_t)dlsym(RTLD_NEXT, "execvp");
    }
    
    // Check if this is a compiler we want to track
    if (is_tracked_compiler(file) && has_c_file(argv)) {
        // Get current environment for logging
        extern char **environ;
        log_compilation(file, argv, environ);
    }
    
    // Call the original execvp
    return original_execvp(file, argv);
}

// Intercept posix_spawn calls
int posix_spawn(pid_t *pid, const char *path,
                const posix_spawn_file_actions_t *file_actions,
                const posix_spawnattr_t *attrp,
                char *const argv[], char *const envp[]) {
    // Initialize if needed
    if (original_posix_spawn == NULL) {
        original_posix_spawn = (posix_spawn_func_t)dlsym(RTLD_NEXT, "posix_spawn");
    }
    
    // Check if this is a compiler we want to track
    if (is_tracked_compiler(path) && has_c_file(argv)) {
        log_compilation(path, argv, envp);
    }
    
    // Call the original posix_spawn
    return original_posix_spawn(pid, path, file_actions, attrp, argv, envp);
}
