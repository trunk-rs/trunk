#[derive(Clone, Debug)]
pub enum TlsConfig {
    #[cfg(any(feature = "rustls", feature = "rustls-aws-lc"))]
    Rustls {
        config: axum_server::tls_rustls::RustlsConfig,
    },
    #[cfg(feature = "native-tls")]
    Native { acceptor: NativeTlsAcceptor },
}

#[cfg(any(feature = "rustls", feature = "rustls-aws-lc"))]
impl From<axum_server::tls_rustls::RustlsConfig> for TlsConfig {
    fn from(config: axum_server::tls_rustls::RustlsConfig) -> Self {
        Self::Rustls { config }
    }
}

#[cfg(feature = "native-tls")]
impl From<NativeTlsAcceptor> for TlsConfig {
    fn from(acceptor: NativeTlsAcceptor) -> Self {
        Self::Native { acceptor }
    }
}

#[cfg(feature = "native-tls")]
pub(crate) fn native_tls_identity(
    cert: &[u8],
    key: &[u8],
) -> anyhow::Result<native_tls_provider::Identity> {
    if key.starts_with(b"-----BEGIN RSA PRIVATE KEY-----") {
        let (_, key) = pkcs8::SecretDocument::from_pem(std::str::from_utf8(key)?)?;
        // Native TLS requires PKCS#8; wrap the RSA key without changing its contents.
        let algorithm = pkcs8::AlgorithmIdentifierRef {
            oid: pkcs8::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1"),
            parameters: Some(pkcs8::der::asn1::AnyRef::NULL),
        };
        let key =
            pkcs8::SecretDocument::try_from(pkcs8::PrivateKeyInfo::new(algorithm, key.as_bytes()))?;
        let key = key.to_pem("PRIVATE KEY", pkcs8::LineEnding::LF)?;
        return Ok(native_tls_provider::Identity::from_pkcs8(
            cert,
            key.as_bytes(),
        )?);
    }

    Ok(native_tls_provider::Identity::from_pkcs8(cert, key)?)
}

#[cfg(feature = "native-tls")]
#[derive(Clone, Debug)]
pub struct NativeTlsAcceptor {
    acceptor: tokio_native_tls::TlsAcceptor,
    handshake_timeout: std::time::Duration,
}

#[cfg(feature = "native-tls")]
impl NativeTlsAcceptor {
    pub fn new(acceptor: native_tls_provider::TlsAcceptor) -> Self {
        Self {
            acceptor: acceptor.into(),
            handshake_timeout: std::time::Duration::from_secs(10),
        }
    }
}

#[cfg(feature = "native-tls")]
impl<I, S> axum_server::accept::Accept<I, S> for NativeTlsAcceptor
where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    S: Send + 'static,
{
    type Stream = tokio_native_tls::TlsStream<I>;
    type Service = S;
    type Future = std::pin::Pin<
        Box<dyn Future<Output = std::io::Result<(Self::Stream, Self::Service)>> + Send + 'static>,
    >;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        let acceptor = self.acceptor.clone();
        let handshake_timeout = self.handshake_timeout;

        Box::pin(async move {
            let stream = tokio::time::timeout(handshake_timeout, acceptor.accept(stream))
                .await
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::TimedOut, err))?
                .map_err(std::io::Error::other)?;
            Ok((stream, service))
        })
    }
}

#[cfg(all(test, feature = "native-tls"))]
mod tests {
    use super::{NativeTlsAcceptor, native_tls_identity};

    #[rstest::rstest]
    #[case::pkcs8(include_bytes!("../examples/yew-tls/self_signed_certs/key.pem"))]
    #[case::pkcs1(include_bytes!("../tests/data/tls-key-pkcs1.pem"))]
    #[tokio::test]
    async fn native_tls_acceptor_completes_a_handshake(#[case] key: &[u8]) -> anyhow::Result<()> {
        let identity = native_tls_identity(
            include_bytes!("../examples/yew-tls/self_signed_certs/cert.pem"),
            key,
        )?;
        let acceptor =
            NativeTlsAcceptor::new(native_tls_provider::TlsAcceptor::builder(identity).build()?);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            axum_server::accept::Accept::accept(&acceptor, stream, ()).await?;
            Ok::<_, std::io::Error>(())
        });

        let mut connector = native_tls_provider::TlsConnector::builder();
        connector.danger_accept_invalid_certs(true);
        let connector = tokio_native_tls::TlsConnector::from(connector.build()?);
        connector
            .connect("localhost", tokio::net::TcpStream::connect(address).await?)
            .await?;
        server.await??;

        Ok(())
    }

    #[test]
    fn native_tls_rejects_invalid_pkcs1() {
        let cert = include_bytes!("../examples/yew-tls/self_signed_certs/cert.pem");
        for key in [
            b"-----BEGIN RSA PRIVATE KEY-----\ninvalid\n-----END RSA PRIVATE KEY-----".as_slice(),
            b"-----BEGIN RSA PRIVATE KEY-----\nMAA=\n-----END RSA PRIVATE KEY-----",
        ] {
            assert!(native_tls_identity(cert, key).is_err());
        }
    }
}
