/*
 * Minimal wrapper around mbedTLS for tiny-hc-tls. Keeps the (large,
 * config-dependent) mbedTLS structs on the C side, so Rust only deals with
 * an opaque pointer. Network I/O is done by Rust through the callbacks.
 */
#include <stdlib.h>

#include "mbedtls/ssl.h"
#include "psa/crypto.h"

struct thc_tls {
    mbedtls_ssl_context ssl;
    mbedtls_ssl_config conf;
};

void thc_tls_free(struct thc_tls *t)
{
    if (t == NULL) {
        return;
    }
    mbedtls_ssl_free(&t->ssl);
    mbedtls_ssl_config_free(&t->conf);
    free(t);
}

/* `sni` may be NULL (e.g. for IP address hosts). Certificates are never verified. */
int thc_tls_new(struct thc_tls **out, const char *sni, void *io,
                mbedtls_ssl_send_t *send, mbedtls_ssl_recv_t *recv)
{
    struct thc_tls *t;
    int ret;

    *out = NULL;
    ret = psa_crypto_init();
    if (ret != PSA_SUCCESS) {
        return ret;
    }
    t = calloc(1, sizeof(*t));
    if (t == NULL) {
        return MBEDTLS_ERR_SSL_ALLOC_FAILED;
    }
    mbedtls_ssl_init(&t->ssl);
    mbedtls_ssl_config_init(&t->conf);

    ret = mbedtls_ssl_config_defaults(&t->conf, MBEDTLS_SSL_IS_CLIENT,
                                      MBEDTLS_SSL_TRANSPORT_STREAM,
                                      MBEDTLS_SSL_PRESET_DEFAULT);
    if (ret == 0) {
        mbedtls_ssl_conf_authmode(&t->conf, MBEDTLS_SSL_VERIFY_NONE);
        ret = mbedtls_ssl_setup(&t->ssl, &t->conf);
    }
    if (ret == 0) {
        ret = mbedtls_ssl_set_hostname(&t->ssl, sni);
    }
    if (ret != 0) {
        thc_tls_free(t);
        return ret;
    }
    mbedtls_ssl_set_bio(&t->ssl, io, send, recv, NULL);
    *out = t;
    return 0;
}

int thc_tls_handshake(struct thc_tls *t)
{
    return mbedtls_ssl_handshake(&t->ssl);
}

int thc_tls_write(struct thc_tls *t, const unsigned char *buf, size_t len)
{
    return mbedtls_ssl_write(&t->ssl, buf, len);
}

/* Returns 0 at end of stream, including a close_notify from the server. */
int thc_tls_read(struct thc_tls *t, unsigned char *buf, size_t len)
{
    int ret = mbedtls_ssl_read(&t->ssl, buf, len);
    return ret == MBEDTLS_ERR_SSL_PEER_CLOSE_NOTIFY ? 0 : ret;
}
