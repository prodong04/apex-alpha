use tokio_tungstenite::connect_async;
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let (ws, _) = connect_async("ws://127.0.0.1:3000/ws").await.unwrap();
    let (_, mut read) = ws.split();
    println!("Connected to WS!");
    let mut count = 0;
    while let Some(Ok(msg)) = read.next().await {
        println!("{}", msg);
        count += 1;
        if count > 15 { break; }
    }
}
