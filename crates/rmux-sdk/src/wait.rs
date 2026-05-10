//! Daemon-backed byte waits and snapshot-polled text wait helpers.

use std::future::Future;
use std::io;
use std::time::Duration;

use rmux_proto::{
    CancelSdkWaitRequest, PaneOutputSubscriptionStart, Request, Response, RmuxError as ProtoError,
    SdkWaitForOutputRequest, SdkWaitId, SdkWaitOutcome,
};

use crate::handles::{connect_transport_to_endpoint, Pane};
use crate::transport::DropGuard;
use crate::{Result, RmuxError};

const WAIT_FOR_BYTES_OPERATION: &str = "wait for pane output bytes";
const WAIT_FOR_TEXT_OPERATION: &str = "wait for pane snapshot text";
const TEXT_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(crate) async fn wait_for_bytes(pane: &Pane, bytes: Vec<u8>) -> Result<()> {
    if bytes.is_empty() {
        return Err(RmuxError::protocol(ProtoError::Server(
            "SDK wait bytes must not be empty".to_owned(),
        )));
    }

    let timeout = resolved_wait_timeout(pane.configured_default_timeout());
    with_wait_timeout(
        WAIT_FOR_BYTES_OPERATION,
        timeout,
        wait_for_bytes_without_timeout(pane, bytes, timeout),
    )
    .await
}

pub(crate) async fn wait_for_text(pane: &Pane, text: String) -> Result<()> {
    if text.is_empty() {
        return Err(RmuxError::protocol(ProtoError::Server(
            "SDK wait text must not be empty".to_owned(),
        )));
    }

    let timeout = resolved_wait_timeout(pane.configured_default_timeout());
    with_wait_timeout(
        WAIT_FOR_TEXT_OPERATION,
        timeout,
        wait_for_text_without_timeout(pane, text),
    )
    .await
}

async fn wait_for_bytes_without_timeout(
    pane: &Pane,
    bytes: Vec<u8>,
    timeout: Option<Duration>,
) -> Result<()> {
    let owner_id = pane.transport().sdk_wait_owner_id();
    let wait_id = pane.transport().allocate_sdk_wait_id();
    let cancel_request = Request::CancelSdkWait(CancelSdkWaitRequest { owner_id, wait_id });
    let cancel_client = connect_transport_to_endpoint(pane.endpoint(), timeout).await?;
    let mut cancel_guard = DropGuard::best_effort(cancel_client, cancel_request);

    let result = pane
        .transport()
        .request(Request::SdkWaitForOutput(SdkWaitForOutputRequest {
            owner_id,
            wait_id,
            target: pane.target().into(),
            bytes,
            start: PaneOutputSubscriptionStart::Now,
        }))
        .await;

    cancel_guard.disarm();
    sdk_wait_response_to_result(result?, wait_id)
}

async fn wait_for_text_without_timeout(pane: &Pane, text: String) -> Result<()> {
    loop {
        let snapshot = pane.snapshot().await?;
        if snapshot.visible_text().contains(&text) {
            return Ok(());
        }
        tokio::time::sleep(TEXT_POLL_INTERVAL).await;
    }
}

async fn with_wait_timeout<F, T>(
    operation: &'static str,
    timeout: Option<Duration>,
    future: F,
) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    match timeout {
        Some(timeout) => tokio::time::timeout(timeout, future)
            .await
            .map_err(|_| wait_timeout_error(operation, timeout))?,
        None => future.await,
    }
}

fn resolved_wait_timeout(default_timeout: Option<Duration>) -> Option<Duration> {
    crate::bootstrap::discovery::resolve_timeout(None, default_timeout)
}

fn wait_timeout_error(operation: &'static str, timeout: Duration) -> RmuxError {
    RmuxError::transport(
        operation,
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "timed out after {}s while {operation}",
                timeout.as_secs_f32()
            ),
        ),
    )
}

fn sdk_wait_response_to_result(response: Response, expected_wait_id: SdkWaitId) -> Result<()> {
    match response {
        Response::SdkWaitForOutput(response)
            if response.wait_id == expected_wait_id
                && response.outcome == SdkWaitOutcome::Matched =>
        {
            Ok(())
        }
        Response::SdkWaitForOutput(response)
            if response.wait_id == expected_wait_id
                && response.outcome == SdkWaitOutcome::Cancelled =>
        {
            Err(RmuxError::protocol(ProtoError::Server(format!(
                "SDK wait {} was cancelled",
                response.wait_id.as_u64()
            ))))
        }
        Response::SdkWaitForOutput(response) => {
            Err(RmuxError::protocol(ProtoError::Server(format!(
                "SDK wait response id {} did not match request id {}",
                response.wait_id.as_u64(),
                expected_wait_id.as_u64()
            ))))
        }
        response => Err(crate::handles::session::unexpected_response(
            "sdk-wait-output",
            response,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TransportClient;
    use rmux_proto::{encode_frame, CancelSdkWaitResponse, FrameDecoder, SdkWaitForOutputResponse};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_request(stream: &mut tokio::io::DuplexStream) -> Request {
        let mut decoder = FrameDecoder::new();
        let mut buffer = [0_u8; 512];

        loop {
            if let Some(request) = decoder
                .next_frame::<Request>()
                .expect("request frame decodes")
            {
                return request;
            }

            let read = stream.read(&mut buffer).await.expect("read request");
            assert_ne!(read, 0, "stream closed before request");
            decoder.push_bytes(&buffer[..read]);
        }
    }

    async fn write_response(stream: &mut tokio::io::DuplexStream, response: Response) {
        let frame = encode_frame(&response).expect("response encodes");
        stream.write_all(&frame).await.expect("write response");
        stream.flush().await.expect("flush response");
    }

    #[tokio::test]
    async fn drop_guard_sends_cancel_request_once_when_wait_future_is_dropped() {
        let (client_stream, mut server_stream) = tokio::io::duplex(4096);
        let client = TransportClient::spawn(client_stream);
        let owner_id = client.sdk_wait_owner_id();
        let wait_id = client.allocate_sdk_wait_id();
        let guard = DropGuard::best_effort(
            client,
            Request::CancelSdkWait(CancelSdkWaitRequest { owner_id, wait_id }),
        );

        drop(guard);

        assert_eq!(
            read_request(&mut server_stream).await,
            Request::CancelSdkWait(CancelSdkWaitRequest { owner_id, wait_id })
        );
        write_response(
            &mut server_stream,
            Response::CancelSdkWait(CancelSdkWaitResponse {
                wait_id,
                removed: true,
            }),
        )
        .await;
    }

    #[tokio::test]
    async fn disarmed_drop_guard_does_not_send_stale_cancel() {
        let (client_stream, mut server_stream) = tokio::io::duplex(4096);
        let client = TransportClient::spawn(client_stream);
        let owner_id = client.sdk_wait_owner_id();
        let mut guard = DropGuard::best_effort(
            client,
            Request::CancelSdkWait(CancelSdkWaitRequest {
                owner_id,
                wait_id: SdkWaitId::new(9),
            }),
        );
        guard.disarm();
        drop(guard);

        let mut buffer = [0_u8; 1];
        let read = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            server_stream.read(&mut buffer),
        )
        .await;
        match read {
            Err(_) => {}
            Ok(Ok(0)) => {}
            Ok(other) => panic!("disarmed guard must not write cancel, got {other:?}"),
        }
    }

    #[test]
    fn sdk_wait_response_rejects_mismatched_wait_id() {
        let result = sdk_wait_response_to_result(
            Response::SdkWaitForOutput(SdkWaitForOutputResponse {
                wait_id: SdkWaitId::new(10),
                outcome: SdkWaitOutcome::Matched,
            }),
            SdkWaitId::new(9),
        );

        match result.expect_err("mismatched wait id must fail") {
            RmuxError::Protocol {
                source: ProtoError::Server(message),
                ..
            } => assert!(message.contains("did not match request id 9")),
            error => panic!("expected protocol mismatch, got {error:?}"),
        }
    }

    #[test]
    fn duration_max_resolves_to_no_timeout_for_wait_operations() {
        assert_eq!(resolved_wait_timeout(Some(Duration::MAX)), None);
    }

    #[tokio::test]
    async fn finite_wait_timeout_surfaces_typed_timeout_error() {
        let error = with_wait_timeout(
            "test wait operation",
            Some(Duration::from_millis(1)),
            std::future::pending::<Result<()>>(),
        )
        .await
        .expect_err("pending wait must time out");

        match error {
            RmuxError::Transport { operation, source } => {
                assert_eq!(operation, "test wait operation");
                assert_eq!(source.kind(), io::ErrorKind::TimedOut);
            }
            other => panic!("expected typed transport timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_timeout_branch_awaits_future_directly() {
        let value = with_wait_timeout("test no timeout", None, async { Ok(7_u8) })
            .await
            .expect("untimed ready future completes");

        assert_eq!(value, 7);
    }
}
