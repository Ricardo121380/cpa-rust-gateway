/* macOS acceptance-only interposition. Loaded into one owned stock gateway process. */
#include <arpa/inet.h>
#include <errno.h>
#include <netdb.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>

static int fixture_getaddrinfo(const char *node, const char *service,
                              const struct addrinfo *hints, struct addrinfo **result) {
    if (node && (!strcmp(node, "auth.kimi.com") ||
                 !strcmp(node, "oidc.us-east-1.amazonaws.com"))) {
        /* Distinct loopback marker: ordinary API and management sockets are untouched. */
        return getaddrinfo("127.0.0.2", service, hints, result);
    }
    return getaddrinfo(node, service, hints, result);
}

static int fixture_connect(int fd, const struct sockaddr *address, socklen_t length) {
    if (address && address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *ipv4 = (const struct sockaddr_in *)address;
        if (ipv4->sin_addr.s_addr == inet_addr("127.0.0.2") && ntohs(ipv4->sin_port) == 443) {
            const char *value = getenv("CPAR_DEVICE_LOOPBACK_PORT");
            char *end = NULL;
            long port = value ? strtol(value, &end, 10) : 0;
            if (!value || !end || *end || port < 1024 || port > 65535) {
                errno = EACCES;
                return -1;
            }
            struct sockaddr_in local = *ipv4;
            local.sin_addr.s_addr = inet_addr("127.0.0.1");
            local.sin_port = htons((unsigned short)port);
            return connect(fd, (const struct sockaddr *)&local, sizeof(local));
        }
    }
    return connect(fd, address, length);
}

#define INTERPOSE(replacement, original) \
    __attribute__((used)) static struct { const void *replace; const void *original; } \
    interpose_##original __attribute__((section("__DATA,__interpose"))) = \
    { (const void *)(replacement), (const void *)(original) }
INTERPOSE(fixture_getaddrinfo, getaddrinfo);
INTERPOSE(fixture_connect, connect);
