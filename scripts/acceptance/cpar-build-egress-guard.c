/* Linux acceptance-only guard. Never install it in the production service.
 * The owned process may resolve only the fixed Build inference/catalog host.
 * Only its resolved TCP/443 addresses and loopback sockets may be connected.
 * Audit records contain event classes, never payloads, tokens or account IDs.
 */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <unistd.h>

static int (*original_getaddrinfo)(const char *, const char *, const struct addrinfo *, struct addrinfo **);
static int (*original_connect)(int, const struct sockaddr *, socklen_t);
static pthread_once_t initialized = PTHREAD_ONCE_INIT;
static pthread_mutex_t address_lock = PTHREAD_MUTEX_INITIALIZER;
static struct { int family; unsigned char address[16]; } allowed[64];
static size_t allowed_count;

static void initialize(void) {
    original_getaddrinfo = dlsym(RTLD_NEXT, "getaddrinfo");
    original_connect = dlsym(RTLD_NEXT, "connect");
}

static int audit(const char *event) {
    const char *path = getenv("CPAR_BUILD_EGRESS_AUDIT");
    if (!path || path[0] != '/') return 0;
    int fd = open(path, O_WRONLY | O_APPEND | O_CREAT | O_CLOEXEC | O_NOFOLLOW, 0600);
    if (fd < 0) return 0;
    struct stat status;
    if (fstat(fd, &status) || !S_ISREG(status.st_mode) || status.st_uid != getuid() ||
        (status.st_mode & 077) || status.st_nlink != 1) {
        close(fd);
        return 0;
    }
    char line[128];
    const char prefix[] = "{\"event\":\"";
    size_t prefix_size = sizeof(prefix) - 1, event_size = strlen(event);
    if (prefix_size + event_size + 4 >= sizeof(line)) { close(fd); return 0; }
    memcpy(line, prefix, prefix_size);
    memcpy(line + prefix_size, event, event_size);
    memcpy(line + prefix_size + event_size, "\"}\n", 3);
    size_t length = prefix_size + event_size + 3;
    ssize_t written = write(fd, line, length);
    close(fd);
    return written == (ssize_t)length;
}

static const unsigned char *address_bytes(const struct sockaddr *address, socklen_t length, size_t *size) {
    if (address && address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        *size = 4;
        return (const unsigned char *)&((const struct sockaddr_in *)address)->sin_addr;
    }
    if (address && address->sa_family == AF_INET6 && length >= sizeof(struct sockaddr_in6)) {
        *size = 16;
        return (const unsigned char *)&((const struct sockaddr_in6 *)address)->sin6_addr;
    }
    return NULL;
}

int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints, struct addrinfo **result) {
    pthread_once(&initialized, initialize);
    if (!original_getaddrinfo) return EAI_SYSTEM;
    if (!node || !strcmp(node, "localhost") || !strcmp(node, "127.0.0.1") || !strcmp(node, "::1"))
        return original_getaddrinfo(node, service, hints, result);
    if (strcmp(node, "cli-chat-proxy.grok.com")) {
        audit(!strcmp(node, "auth.x.ai") ? "dns_oauth_denied" : "dns_other_denied");
        if (result) *result = NULL;
        return EAI_FAIL;
    }
    if (!audit("dns_build_allowed")) return EAI_FAIL;
    int status = original_getaddrinfo(node, service, hints, result);
    if (status) return status;
    pthread_mutex_lock(&address_lock);
    for (const struct addrinfo *row = *result; row; row = row->ai_next) {
        size_t size = 0;
        const unsigned char *bytes = address_bytes(row->ai_addr, row->ai_addrlen, &size);
        if (!bytes) continue;
        size_t index = 0;
        while (index < allowed_count && (allowed[index].family != row->ai_family ||
               memcmp(allowed[index].address, bytes, size))) index++;
        if (index == allowed_count && allowed_count < sizeof(allowed) / sizeof(allowed[0])) {
            allowed[allowed_count].family = row->ai_family;
            memcpy(allowed[allowed_count++].address, bytes, size);
        }
    }
    pthread_mutex_unlock(&address_lock);
    return status;
}

static int is_tcp_socket(int fd) {
    int type = 0, protocol = 0;
    socklen_t type_length = sizeof(type), protocol_length = sizeof(protocol);
    return !getsockopt(fd, SOL_SOCKET, SO_TYPE, &type, &type_length) && type == SOCK_STREAM &&
           !getsockopt(fd, SOL_SOCKET, SO_PROTOCOL, &protocol, &protocol_length) && protocol == IPPROTO_TCP;
}

int connect(int fd, const struct sockaddr *address, socklen_t length) {
    pthread_once(&initialized, initialize);
    if (!original_connect) { errno = EACCES; return -1; }
    if (address && address->sa_family == AF_UNIX) return original_connect(fd, address, length);
    size_t size = 0;
    const unsigned char *bytes = address_bytes(address, length, &size);
    unsigned short port = 0;
    if (bytes && address->sa_family == AF_INET) {
        if (bytes[0] == 127) return original_connect(fd, address, length);
        port = ntohs(((const struct sockaddr_in *)address)->sin_port);
    } else if (bytes && address->sa_family == AF_INET6) {
        const unsigned char loopback[16] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
        if (!memcmp(bytes, loopback, 16)) return original_connect(fd, address, length);
        port = ntohs(((const struct sockaddr_in6 *)address)->sin6_port);
    }
    int admitted = 0, tcp = bytes && is_tcp_socket(fd);
    pthread_mutex_lock(&address_lock);
    for (size_t index = 0; tcp && port == 443 && index < allowed_count; index++) {
        if (allowed[index].family == address->sa_family && !memcmp(allowed[index].address, bytes, size)) {
            admitted = 1;
            break;
        }
    }
    pthread_mutex_unlock(&address_lock);
    if (!admitted || !audit("connect_build_allowed")) {
        audit("connect_other_denied");
        errno = EACCES;
        return -1;
    }
    return original_connect(fd, address, length);
}
