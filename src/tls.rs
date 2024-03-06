#[derive(Clone, Debug)]
pub enum TlsConfig {
    #[cfg(feature = "rustls")]
    Rustls {
        config: axum_server::tls_rustls::RustlsConfig,
    },
    #[cfg(feature = "native-tls")]
    Native {
        config: axum_server::tls_openssl::OpenSSLConfig,
    },
}

#[cfg(feature = "rustls")]
impl From<axum_server::tls_rustls::RustlsConfig> for TlsConfig {
    fn from(config: axum_server::tls_rustls::RustlsConfig) -> Self {
        Self::Rustls { config }
    }
}

#[cfg(feature = "native-tls")]
impl From<axum_server::tls_openssl::OpenSSLConfig> for TlsConfig {
    fn from(config: axum_server::tls_openssl::OpenSSLConfig) -> Self {
        Self::Native { config }
    }
}
