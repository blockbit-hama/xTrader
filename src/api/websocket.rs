use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::broadcast;

use crate::api::models::WebSocketMessage;
use crate::matching_engine::model::ExecutionReport;
use crate::server::ServerState;

/// WebSocket 연결 핸들러
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> Response {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}

/// WebSocket 연결 처리
async fn websocket_connection(
    socket: WebSocket,
    state: ServerState,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.execution_tx.subscribe();

    // 클라이언트로부터 메시지 수신 처리
    let send_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if let Ok(msg) = msg {
                match msg {
                    Message::Text(text) => {
                        // 클라이언트로부터 받은 텍스트 메시지 처리
                        println!("Received message: {}", text);
                    }
                    Message::Close(_) => {
                        println!("WebSocket connection closed");
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    // WebSocket 메시지 브로드캐스트 수신 및 클라이언트로 전송
    let recv_task = tokio::spawn(async move {
        while let Ok(ws_message) = rx.recv().await {
            let json_message = serde_json::to_string(&ws_message).unwrap();
            
            if sender.send(Message::Text(json_message)).await.is_err() {
                break;
            }
        }
    });

    // 두 태스크 중 하나라도 완료되면 연결 종료
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}