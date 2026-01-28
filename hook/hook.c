#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dlfcn.h>
#include <sys/file.h>
#include <limits.h>

// Function pointer type for execve
typedef int (*execve_func_t)(const char *pathname, char *const argv[], char *const envp[]);

// Original execve function
static execve_func_t original_execve = NULL;

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
    
    // Check if abs_path starts with project_root
    size_t root_len = strlen(project_root);
    return strncmp(abs_path, project_root, root_len) == 0;
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
    
    // Open output file with append mode
    FILE *fp = fopen(output_file, "a");
    if (!fp) {
        return;
    }
    
    // Lock file for concurrent writes
    int fd = fileno(fp);
    flock(fd, LOCK_EX);
    
    // Get current working directory
    char cwd[PATH_MAX];
    if (getcwd(cwd, sizeof(cwd)) == NULL) {
        flock(fd, LOCK_UN);
        fclose(fp);
        return;
    }
    
    // Write entry marker
    fprintf(fp, "---ENTRY---\n");
    
    // Write compile options (skip first arg which is the compiler)
    int first_option = 1;
    for (int i = 1; argv[i] != NULL; i++) {
        const char *arg = argv[i];
        
        // Only include preprocessing-related flags and the source file
        if (arg[0] == '-') {
            // Include -I, -D, -U, -std, -include flags
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
                
                // If -I, -D, -U, or -include have a separate argument, include it
                if ((strcmp(arg, "-I") == 0 || 
                     strcmp(arg, "-D") == 0 || 
                     strcmp(arg, "-U") == 0 ||
                     strcmp(arg, "-include") == 0) && argv[i + 1] != NULL) {
                    fprintf(fp, " %s", argv[i + 1]);
                    i++; // Skip next argument as we've already processed it
                }
            }
        } else if (ends_with(arg, ".c")) {
            // Found the C file - convert to absolute path if needed
            char abs_file[PATH_MAX];
            if (arg[0] == '/') {
                strcpy(abs_file, arg);
            } else {
                snprintf(abs_file, PATH_MAX, "%s/%s", cwd, arg);
            }
            
            // Only track if file is in project root
            if (is_in_project_root(abs_file, project_root)) {
                // Write the absolute file path on a new line
                fprintf(fp, "\n%s\n", abs_file);
                
                // Write the working directory
                fprintf(fp, "%s\n", cwd);
            }
        }
    }
    
    flock(fd, LOCK_UN);
    fclose(fp);
}

// Intercept execve calls
int execve(const char *pathname, char *const argv[], char *const envp[]) {
    // Initialize original_execve if not done yet
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
