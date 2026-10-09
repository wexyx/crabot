#include <openssl/ssl.h>
#include <openssl/opensslv.h>

int main(void) {
    SSL_CTX *context = SSL_CTX_new(TLS_method());
    if (context == NULL || OPENSSL_version_major() != OPENSSL_VERSION_MAJOR) {
        SSL_CTX_free(context);
        return 1;
    }
    SSL_CTX_free(context);
    return 0;
}
