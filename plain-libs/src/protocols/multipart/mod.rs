// Streaming parser adapted from multer 3.1.0 (https://github.com/rwf2/multer), see LICENSE.

use axum::{RequestExt, body::BodyDataStream, extract::Request};
use bytes::{Buf, Bytes, BytesMut};
use futures_core::Stream;
use http::{
    HeaderMap, HeaderName, HeaderValue,
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
};
use memchr::{memmem, memrchr};
use std::{
    fmt,
    future::poll_fn,
    pin::Pin,
    task::{Context, Poll},
};

mod disposition;
use disposition::ContentDispositionAttr;

#[derive(Debug)]
pub enum MultipartError {
    Headers(httparse::Error),
    HeaderName(http::header::InvalidHeaderName),
    HeaderValue(http::header::InvalidHeaderValue),
    Body(axum::Error),
    Incomplete,
}

impl fmt::Display for MultipartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Error parsing `multipart/form-data` request")
    }
}

impl std::error::Error for MultipartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Headers(error) => Some(error),
            Self::HeaderName(error) => Some(error),
            Self::HeaderValue(error) => Some(error),
            Self::Body(error) => Some(error),
            Self::Incomplete => None,
        }
    }
}

type Result<T> = std::result::Result<T, MultipartError>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    FirstBoundary,
    Boundary,
    BoundaryType,
    Padding,
    Headers,
    Data,
    End,
}

pub struct Multipart {
    stream: BodyDataStream,
    buffer: BytesMut,
    boundary: String,
    stage: Stage,
    eof: bool,
}

impl Multipart {
    pub fn from_request(request: Request) -> Option<Self> {
        let content_type = request.headers().get(CONTENT_TYPE)?.to_str().ok()?;
        let mime = content_type.parse::<mime::Mime>().ok()?;
        if mime.type_() != mime::MULTIPART || mime.subtype() != mime::FORM_DATA {
            return None;
        }
        let boundary = mime.get_param(mime::BOUNDARY)?;
        Some(Self {
            stream: request.with_limited_body().into_body().into_data_stream(),
            buffer: BytesMut::new(),
            boundary: format!("\r\n--{boundary}"),
            stage: Stage::FirstBoundary,
            eof: false,
        })
    }

    pub async fn next_field(&mut self) -> Result<Option<Field<'_>>> {
        let headers = poll_fn(|cx| self.poll_next_headers(cx)).await?;
        Ok(headers.map(|headers| Field::new(self, headers)))
    }

    fn poll_next_headers(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<HeaderMap>>> {
        if self.stage == Stage::End {
            return Poll::Ready(Ok(None));
        }
        self.poll_stream(cx)?;

        if self.stage == Stage::FirstBoundary {
            let Some(index) = memmem::find(&self.buffer, &self.boundary.as_bytes()[2..]) else {
                return self.need_data();
            };
            self.buffer.advance(index);
            self.stage = Stage::Boundary;
        }

        if self.stage == Stage::Data {
            match self.read_field_data()? {
                Some((true, _)) => self.stage = Stage::Boundary,
                _ => return Poll::Pending,
            }
        }

        if self.stage == Stage::Boundary {
            let boundary = &self.boundary.as_bytes()[2..];
            let Some(bytes) = self.buffer.get(..boundary.len()) else {
                return self.need_data();
            };
            if bytes != boundary {
                return Poll::Ready(Err(MultipartError::Incomplete));
            }
            self.buffer.advance(boundary.len());
            self.stage = Stage::BoundaryType;
        }

        if self.stage == Stage::BoundaryType {
            let Some(bytes) = self.buffer.get(..2) else {
                return self.need_data();
            };
            if bytes == b"--" {
                self.stage = Stage::End;
                return Poll::Ready(Ok(None));
            }
            self.stage = Stage::Padding;
        }

        if self.stage == Stage::Padding {
            let Some(index) = self
                .buffer
                .iter()
                .position(|byte| !matches!(byte, b' ' | b'\t'))
            else {
                self.buffer.clear();
                return self.need_data();
            };
            self.buffer.advance(index);
            let Some(bytes) = self.buffer.get(..2) else {
                return self.need_data();
            };
            if bytes != b"\r\n" {
                return Poll::Ready(Err(MultipartError::Incomplete));
            }
            self.buffer.advance(2);
            self.stage = Stage::Headers;
        }

        let Some(index) = memmem::find(&self.buffer, b"\r\n\r\n") else {
            return self.need_data();
        };
        let bytes = self.buffer.split_to(index + 4);
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let parsed =
            match httparse::parse_headers(&bytes, &mut headers).map_err(MultipartError::Headers)? {
                httparse::Status::Complete((_, headers)) => headers,
                httparse::Status::Partial => return Poll::Ready(Err(MultipartError::Incomplete)),
            };
        let mut headers = HeaderMap::with_capacity(parsed.len());
        for header in parsed {
            let name = HeaderName::try_from(header.name).map_err(MultipartError::HeaderName)?;
            let value = HeaderValue::try_from(header.value).map_err(MultipartError::HeaderValue)?;
            headers.insert(name, value);
        }
        self.stage = Stage::Data;
        Poll::Ready(Ok(Some(headers)))
    }

    fn need_data<T>(&self) -> Poll<Result<T>> {
        if self.eof {
            Poll::Ready(Err(MultipartError::Incomplete))
        } else {
            Poll::Pending
        }
    }

    fn poll_stream(&mut self, cx: &mut Context<'_>) -> Result<()> {
        while !self.eof {
            match Pin::new(&mut self.stream).poll_next(cx) {
                Poll::Ready(Some(Ok(data))) => self.buffer.extend_from_slice(&data),
                Poll::Ready(Some(Err(error))) => return Err(MultipartError::Body(error)),
                Poll::Ready(None) => self.eof = true,
                Poll::Pending => break,
            }
        }
        Ok(())
    }

    fn read_field_data(&mut self) -> Result<Option<(bool, Bytes)>> {
        if let Some(index) = memmem::find(&self.buffer, self.boundary.as_bytes()) {
            let bytes = self.buffer.split_to(index).freeze();
            self.buffer.advance(2);
            return Ok(Some((true, bytes)));
        }
        if self.eof {
            return Err(MultipartError::Incomplete);
        }
        if self.buffer.is_empty() {
            return Ok(None);
        }

        let tail = self.buffer.len().saturating_sub(self.boundary.len() - 1);
        let remaining = &self.buffer[tail..];
        if let Some(index) = memrchr(b'\r', remaining) {
            let start = tail + index;
            if memmem::find(self.boundary.as_bytes(), &self.buffer[start..]).is_some() {
                return Ok((start != 0).then(|| (false, self.buffer.split_to(start).freeze())));
            }
        }
        Ok(Some((false, self.buffer.split().freeze())))
    }
}

pub struct Field<'a> {
    multipart: &'a mut Multipart,
    name: Option<String>,
    file_name: Option<String>,
    content_type: Option<mime::Mime>,
}

impl<'a> Field<'a> {
    fn new(multipart: &'a mut Multipart, headers: HeaderMap) -> Self {
        let disposition = headers.get(CONTENT_DISPOSITION).map(HeaderValue::as_bytes);
        let attr = |kind: ContentDispositionAttr| {
            kind.extract_from(disposition?)
                .map(std::borrow::Cow::into_owned)
        };
        Self {
            multipart,
            name: attr(ContentDispositionAttr::Name),
            file_name: attr(ContentDispositionAttr::FileName),
            content_type: headers
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok()),
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_ref().map(AsRef::as_ref)
    }

    pub async fn chunk(&mut self) -> Result<Option<Bytes>> {
        poll_fn(|cx| {
            let multipart = &mut self.multipart;
            if multipart.stage != Stage::Data {
                return Poll::Ready(Ok(None));
            }
            multipart.poll_stream(cx)?;
            let Some((done, bytes)) = multipart.read_field_data()? else {
                return Poll::Pending;
            };
            if done {
                multipart.stage = Stage::Boundary;
            }
            Poll::Ready(Ok(Some(bytes)))
        })
        .await
    }

    pub async fn bytes(mut self) -> Result<Bytes> {
        let mut bytes = BytesMut::new();
        while let Some(chunk) = self.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes.freeze())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/protocols/multipart.rs"]
mod tests;
