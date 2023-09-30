use std::io::{Read, Seek, SeekFrom};

use tokio_util::sync::CancellationToken;

use stream_download::http::reqwest;
use stream_download::http::HttpStream;
use stream_download::source::SourceStream;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};

use symphonia::core::io::MediaSource;

use crate::{Cancellable, Error};

pub struct StreamDownloadFile(StreamDownload<TempStorageProvider>, Option<u64>);

impl Read for StreamDownloadFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for StreamDownloadFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl MediaSource for StreamDownloadFile {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.1
    }
}

impl Cancellable for StreamDownloadFile {
    // We can't just clone the inner cancellation token
    //  because that would get cancelled when download is complete and it is dropped
    fn cancel_token(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let t1 = token.clone();
        let t2 = self.0.download_task_cancellation_token.clone();
        tokio::spawn(async move {
            t1.cancelled().await;
            t2.cancel();
        });
        token
    }
}

impl StreamDownloadFile {
    pub async fn from_url(url: impl AsRef<str>) -> Result<Self, Error> {
        use reqwest::header;
        // TODO: make this client reusable
        let mut headers = header::HeaderMap::new();
        // Add a user agent header because catbox got ddosed or something
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("shiroky-bot"),
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(Error::ClientError)?;
        let url = url.as_ref().parse().map_err(Error::UrlParseError)?;
        log::debug!("Creating audio input from url: {}", &url);

        let stream = HttpStream::new(client, url)
            .await
            .map_err(Error::StreamDownloadErr)?;
        let content_length = stream.content_length();

        let file = StreamDownload::from_stream(
            stream,
            TempStorageProvider::default(),
            Settings::default(),
        )
        .await
        .map_err(Error::StreamDownloadErr)?;
        let stream = StreamDownloadFile(file, content_length);

        Ok(stream)
    }
}
