use anyhow::{Context, Result, ensure};
use std::{fs::File, io::Read, path::Path, time::Duration};
use tonic::transport::{ClientTlsConfig, Endpoint};

const TEST_ENDPOINT: &str = "https://cabinet.tax.gov.ua:9443";
const STATUS_METHOD: &str = "/com.programika.rro.ws.chk.ChkIncomeService/statusRro";
const MAX_SIGNED_BYTES: usize = 256 * 1024;

#[derive(Clone, PartialEq, prost::Message)]
struct CheckRequest {
    #[prost(bytes = "vec", tag = "3")]
    rro_fn_sign: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct StatusResponse {
    #[prost(bool, tag = "1")]
    open_shift: bool,
    #[prost(bool, tag = "2")]
    online: bool,
    #[prost(int32, tag = "4")]
    status: i32,
    #[prost(string, tag = "5")]
    error_message: String,
}

pub fn test_status(path: &Path) -> Result<String> {
    let mut signed = Vec::new();
    let file = File::open(path).context("Не вдалося відкрити файл підписаного запиту")?;
    file.take((MAX_SIGNED_BYTES + 1) as u64)
        .read_to_end(&mut signed)?;
    ensure!(
        signed.len() <= MAX_SIGNED_BYTES,
        "Файл підписаного запиту завеликий"
    );
    ensure!(
        is_signed_data(&signed),
        "Потрібен бінарний CMS SignedData із підписаним номером ПРРО"
    );
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let channel = Endpoint::from_static(TEST_ENDPOINT)
                .tls_config(ClientTlsConfig::new().with_webpki_roots())?
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .connect()
                .await
                .context("Тестовий сервер ДПС недоступний")?;
            let mut client = tonic::client::Grpc::new(channel);
            client
                .ready()
                .await
                .context("gRPC-з’єднання з ДПС недоступне")?;
            let mut request = tonic::Request::new(CheckRequest {
                rro_fn_sign: signed,
            });
            request.set_timeout(Duration::from_secs(10));
            let response: tonic::Response<StatusResponse> = client
                .unary(
                    request,
                    STATUS_METHOD.parse()?,
                    tonic_prost::ProstCodec::default(),
                )
                .await
                .context("ДПС не повернула статус ПРРО")?;
            describe(response.into_inner())
        })
}

fn is_signed_data(bytes: &[u8]) -> bool {
    // DER ContentInfo starts with a sequence followed by the SignedData OID.
    const OID: &[u8] = &[
        0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02,
    ];
    let [0x30, length, rest @ ..] = bytes else {
        return false;
    };
    let header = if *length < 0x80 {
        0
    } else {
        usize::from(length & 0x7f)
    };
    if *length == 0x80 || header > 3 || rest.len() < header {
        return false;
    }
    let content_length = if header == 0 {
        usize::from(*length)
    } else {
        rest[..header]
            .iter()
            .fold(0, |value, byte| (value << 8) | usize::from(*byte))
    };
    let content = &rest[header..];
    content.len() == content_length
        && content.starts_with(OID)
        && content.get(OID.len()) == Some(&0xa0)
}

fn describe(response: StatusResponse) -> Result<String> {
    ensure!(
        response.status == 1,
        "Тестова ДПС відхилила запит (код {}): {}",
        response.status,
        response.error_message
    );
    let shift = if response.open_shift {
        "відкрита"
    } else {
        "закрита"
    };
    let mode = if response.online {
        "онлайн"
    } else {
        "офлайн"
    };
    Ok(format!("Тестова ДПС · зміна {shift} · {mode}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_positive_server_status_reports_shift() {
        let response = StatusResponse {
            open_shift: true,
            online: true,
            status: 1,
            error_message: String::new(),
        };
        assert_eq!(
            describe(response).unwrap(),
            "Тестова ДПС · зміна відкрита · онлайн"
        );
        assert!(
            describe(StatusResponse {
                status: -14,
                ..Default::default()
            })
            .is_err()
        );
        assert!(describe(StatusResponse::default()).is_err());
    }

    #[test]
    fn rejects_unsigned_and_truncated_input() {
        assert!(!is_signed_data(b"1234567890"));
        assert!(!is_signed_data(&[0x30, 0x82, 0xff]));
        assert!(!is_signed_data(&[0x30, 0x03, 0x02, 0x01, 0x00]));
    }
}
