

use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use worker::*;
use futures::StreamExt;
use std::pin::Pin;
use std::task::{Context, Poll};

static BUFSIZE: usize = 4096;

pin_project! {
    pub struct WsStream<'a> {
        ws: &'a  WebSocket,
        events: EventStream<'a>,
        read_buffer: Vec<u8>,
        write_buffer: Vec<u8>,
        is_closed: bool,
    }   
}

impl<'a> WsStream<'a> {
    pub fn new(ws: &'a WebSocket, events: EventStream<'a>, early_data: Option<Vec<u8>>) -> Self {
        let mut s  = Self { 
            ws, 
            events ,
            read_buffer: Vec::with_capacity(BUFSIZE),
            write_buffer: Vec::with_capacity(BUFSIZE),
            is_closed: false,
        };
        if let Some(early_data) = early_data {
            s.read_buffer.extend(early_data);
        }
        s
    }
}

impl<'a> AsRef<WsStream<'a>> for WsStream<'a> {
    fn as_ref(&self) -> &WsStream<'a> {
        self
    }
}

impl<'a> AsMut<WsStream<'a>> for WsStream<'a> {
    fn as_mut(&mut self) -> &mut WsStream<'a> {
        self
    }
}

impl<'a> AsyncRead for WsStream<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<tokio::io::Result<()>> {

        let this = self.project();

        if !this.read_buffer.is_empty() {
            let to_copy = std::cmp::min(this.read_buffer.len(), buf.remaining());
            let data = this.read_buffer.drain(..to_copy).collect::<Vec<u8>>();
            buf.put_slice(&data);
            return Poll::Ready(Ok(()));
        }
        if *this.is_closed {
            return Poll::Ready(Ok(())); 
        }
        match this.events.poll_next_unpin(cx) {
            Poll::Ready(msgoption)=>{
                match msgoption{
                    Some(Ok(message))=>{
                        match message {
                            worker::WebsocketEvent::Message(msg) => {
                                let data = msg.bytes().unwrap();
                                let to_copy = std::cmp::min( data.len(), buf.remaining());
                                buf.put_slice(&data[..to_copy]);
                                                        
                                if data.len() > to_copy {
                                    this.read_buffer.extend_from_slice(&data[to_copy..]);
                                }
                            }
                            worker::WebsocketEvent::Close(_) => {
                                *this.is_closed = true;
                            }
                        }
                        return Poll::Ready(Ok(()));
                    }
                    Some(Err(e))=>{
                        Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("WebSocket error: {}", e),
                        ))) 
                    }
                    None=>{
                        *this.is_closed = true;
                        return Poll::Ready(Ok(()));
                    }
                }
            }
            Poll::Pending => return Poll::Pending,
        }
    }   
}

impl<'a> AsyncWrite for WsStream<'a>  {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<tokio::io::Result<usize>> {

        let this = self.project();
        this.write_buffer.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
        let this = self.project();
        
        if this.write_buffer.is_empty() {
            return Poll::Ready(Ok(()));
        }
        
        match this.ws.send_with_bytes(&this.write_buffer) {
            Ok(()) => {
                this.write_buffer.clear();
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("WebSocket send error: {}", e),
            ))),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
        let this = self.project();

        match this.ws.as_ref().close() {
            Ok(()) => Poll::Ready(Ok(())),
            Err(_e) => Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("WebSocket write close failed"),
            ))),
        }
    }
}
