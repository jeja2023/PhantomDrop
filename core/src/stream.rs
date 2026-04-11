use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::{self, Stream};
use serde::Serialize;
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::broadcast;

/**
 * 幻影中台 - WebSocket/SSE 实时流通道 (Stream)
 * 职责：将后端捕获的邮件事件秒级推送到前端面板
 */

#[derive(Serialize, Clone, Debug)]
pub struct StreamPayload {
    pub id: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

pub struct StreamHub {
    pub tx: broadcast::Sender<StreamPayload>,
}

impl StreamHub {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(100);
        Arc::new(Self { tx })
    }

    /// 发送全局通知
    pub fn broadcast(&self, payload: StreamPayload) {
        let _ = self.tx.send(payload);
    }
}

/// SSE 核心处理器
pub async fn sse_handler(
    State(hub): State<Arc<StreamHub>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = hub.tx.subscribe();

    let stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(payload) => {
                let event = Event::default()
                    .event(payload.event_type)
                    .data(serde_json::to_string(&payload.data).unwrap());
                Some((Ok(event), rx))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default().interval(Duration::from_secs(15)))
}
